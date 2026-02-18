use crate::client::SearchfoxClient;
use crate::types::{BlameInfo, CommitInfo, ParsedCommitInfo};
use crate::utils::searchfox_url_repo;
use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashMap;

impl SearchfoxClient {
    pub async fn get_head_hash(&self) -> anyhow::Result<String> {
        let url = format!(
            "https://searchfox.org/{}/commit-info/HEAD",
            searchfox_url_repo(&self.repo)
        );
        let response = self.get_raw(&url).await?;
        let json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|_| anyhow::anyhow!("Failed to parse HEAD commit info"))?;
        json.as_array()
            .and_then(|arr| arr.first())
            .and_then(|commit| commit.get("parent"))
            .and_then(|p| p.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Could not find HEAD revision hash in commit-info"))
    }

    /// Fetch blame data for specific lines in a file
    pub async fn get_blame_for_lines(
        &self,
        path: &str,
        lines: &[usize],
    ) -> Result<HashMap<usize, BlameInfo>> {
        // Fetch the HTML page for the file
        let url = format!("https://searchfox.org/{}/source/{}", self.repo, path);
        let html = self.get_html(&url).await?;

        // Parse blame data from HTML
        let blame_map = Self::parse_blame_from_html(&html)?;

        // Filter to only the requested lines
        let filtered_blame: HashMap<usize, (String, String, usize)> = blame_map
            .into_iter()
            .filter(|(line_no, _)| lines.contains(line_no))
            .collect();

        if filtered_blame.is_empty() {
            return Ok(HashMap::new());
        }

        // Collect unique commit hashes
        let unique_commits: Vec<&str> = {
            let mut commits: Vec<&str> = filtered_blame
                .values()
                .map(|(hash, _, _)| hash.as_str())
                .collect();
            commits.sort_unstable();
            commits.dedup();
            commits
        };

        // Batch fetch commit info
        let commit_infos = self.get_commit_info(&unique_commits).await?;

        // Build commit hash -> CommitInfo map
        let commit_map: HashMap<String, CommitInfo> = unique_commits
            .into_iter()
            .zip(commit_infos.into_iter())
            .map(|(hash, info)| (hash.to_string(), info))
            .collect();

        // Build final result
        let result = filtered_blame
            .into_iter()
            .map(|(line_no, (hash, path, orig_line))| {
                let commit_info = commit_map.get(&hash).cloned();
                let blame_info = BlameInfo {
                    commit_hash: hash.clone(),
                    original_path: path,
                    original_line: orig_line,
                    commit_info,
                };
                (line_no, blame_info)
            })
            .collect();

        Ok(result)
    }

    /// Fetch commit info for commit hashes (batched to avoid 414 URI Too Long)
    async fn get_commit_info(&self, revs: &[&str]) -> Result<Vec<CommitInfo>> {
        if revs.is_empty() {
            return Ok(Vec::new());
        }

        // Batch requests to avoid hitting URL length limits (414 errors)
        // Each hash is 40 chars + 1 comma, so ~50 hashes should be safe
        const BATCH_SIZE: usize = 50;

        let mut all_infos = Vec::new();

        for chunk in revs.chunks(BATCH_SIZE) {
            let revs_str = chunk.join(",");
            let url = format!(
                "https://searchfox.org/{}/commit-info/{}",
                self.repo, revs_str
            );

            let response = self.get_raw(&url).await?;
            let mut commit_infos: Vec<CommitInfo> = serde_json::from_str(&response)?;
            all_infos.append(&mut commit_infos);
        }

        Ok(all_infos)
    }

    /// Parse blame data from HTML, returns map of line -> (commit_hash, path, original_line)
    fn parse_blame_from_html(html: &str) -> Result<HashMap<usize, (String, String, usize)>> {
        let document = Html::parse_document(html);
        let blame_selector = Selector::parse(".blame-strip").unwrap();
        let line_selector = Selector::parse("div[role='row']").unwrap();

        let mut result = HashMap::new();
        let mut line_number = 1;

        // The searchfox HTML structure has rows with role="row"
        // Each row contains a blame-strip div and code
        for row in document.select(&line_selector) {
            // Try to find a blame-strip in this row
            if let Some(blame_elem) = row.select(&blame_selector).next() {
                if let Some(blame_data) = blame_elem.value().attr("data-blame") {
                    if let Some((hash, path, orig_line)) = Self::parse_data_blame(blame_data) {
                        result.insert(line_number, (hash, path, orig_line));
                    }
                }
            }
            line_number += 1;
        }

        log::debug!("Parsed {} blame entries from HTML", result.len());
        Ok(result)
    }

    /// Parse data-blame attribute format: "hash#path#lineno"
    /// % in path means "same as current file"
    fn parse_data_blame(data: &str) -> Option<(String, String, usize)> {
        let parts: Vec<&str> = data.split('#').collect();
        if parts.len() != 3 {
            return None;
        }

        let hash = parts[0].to_string();
        let path = parts[1].to_string();
        let line_no = parts[2].parse::<usize>().ok()?;

        Some((hash, path, line_no))
    }
}

/// Parse commit header HTML to extract structured information
pub fn parse_commit_header(header: &str) -> ParsedCommitInfo {
    // Remove HTML tags for parsing
    let text = strip_html_tags(header);

    // Try to extract bug number
    let bug_number = extract_bug_number(&text);

    // Split by newline or <br> to separate message from author/date
    let parts: Vec<&str> = text.split('\n').collect();

    let message = if let Some(first_part) = parts.first() {
        // Remove "Bug XXXXXX: " prefix if present
        if let Some(idx) = first_part.find(':') {
            first_part[idx + 1..].trim().to_string()
        } else {
            first_part.trim().to_string()
        }
    } else {
        String::new()
    };

    // Second part should contain author and date
    let (author, date) = if parts.len() > 1 {
        parse_author_date(parts[1])
    } else {
        (String::new(), String::new())
    };

    ParsedCommitInfo {
        bug_number,
        message,
        author,
        date,
    }
}

fn strip_html_tags(html: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    let without_tags = tag_re.replace_all(html, "");

    // Decode common HTML entities
    without_tags
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn extract_bug_number(text: &str) -> Option<u64> {
    let bug_re = Regex::new(r"[Bb]ug\s+(\d+)").unwrap();
    bug_re
        .captures(text)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
}

fn parse_author_date(text: &str) -> (String, String) {
    // Format is typically "Author, Date"
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() >= 2 {
        let author = parts[0].trim().to_string();
        let date = parts[1..].join(",").trim().to_string();
        (author, date)
    } else {
        (text.trim().to_string(), String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_blame() {
        let data = "88a286dcec9ba069397bd4c4c35b3e317bf66f4f#%#7";
        let result = SearchfoxClient::parse_data_blame(data);
        assert!(result.is_some());

        let (hash, path, line) = result.unwrap();
        assert_eq!(hash, "88a286dcec9ba069397bd4c4c35b3e317bf66f4f");
        assert_eq!(path, "%");
        assert_eq!(line, 7);
    }

    #[test]
    fn test_parse_commit_header() {
        let header =
            "Bug <a href=\"...\">123456</a>: Fix audio issue\n<br><i>John Doe, 2021-05-15</i>";
        let result = parse_commit_header(header);

        assert_eq!(result.bug_number, Some(123456));
        assert_eq!(result.message, "Fix audio issue");
        assert_eq!(result.author, "John Doe");
        assert_eq!(result.date, "2021-05-15");
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "Bug <a href=\"url\">123</a>: message";
        let result = strip_html_tags(html);
        assert_eq!(result, "Bug 123: message");
    }
}
