use crate::client::SearchfoxClient;
use crate::search::SearchOptions;
use crate::utils::{
    extract_complete_method, find_symbol_in_local_content, get_github_raw_url,
    is_mozilla_repository, read_local_file,
};
use anyhow::Result;
use log::{debug, error};

impl SearchfoxClient {
    pub async fn get_definition_context(
        &self,
        file_path: &str,
        line_number: usize,
        context_lines: usize,
        symbol_name: Option<&str>,
    ) -> Result<String> {
        if is_mozilla_repository() {
            if let Some(local_content) = read_local_file(file_path) {
                let lines: Vec<&str> = local_content.lines().collect();

                let actual_line = if line_number > 0 && line_number <= lines.len() {
                    let line_idx = line_number - 1;
                    let line_content = lines[line_idx];

                    let looks_correct = if let Some(symbol) = symbol_name {
                        line_content.contains(symbol)
                            || (symbol.contains("::")
                                && line_content.contains(symbol.split("::").last().unwrap_or("")))
                    } else {
                        line_content.contains("::") || line_content.contains("(")
                    };

                    if looks_correct {
                        Some(line_number)
                    } else if let Some(symbol) = symbol_name {
                        find_symbol_in_local_content(&local_content, line_number, symbol)
                    } else {
                        None
                    }
                } else if let Some(symbol) = symbol_name {
                    find_symbol_in_local_content(&local_content, 1, symbol)
                } else {
                    None
                };

                let final_line = actual_line.unwrap_or(line_number);

                let (_, method_lines) = extract_complete_method(&lines, final_line);

                if method_lines.len() > 1 {
                    return Ok(method_lines.join("\n"));
                }

                let start_line = if final_line > context_lines {
                    final_line - context_lines
                } else {
                    1
                };
                let end_line = std::cmp::min(final_line + context_lines, lines.len());

                let mut result = String::new();
                for (i, line) in lines.iter().enumerate() {
                    let line_num = i + 1;
                    if line_num >= start_line && line_num <= end_line {
                        let marker = if line_num == final_line { ">>>" } else { "   " };
                        result.push_str(&format!("{marker} {line_num:4}: {line}\n"));
                    }
                }

                return Ok(result);
            }
        }

        let github_url = get_github_raw_url(&self.repo, file_path);
        let file_content = self.get_raw(&github_url).await?;
        let lines: Vec<&str> = file_content.lines().collect();

        let (_, method_lines) = extract_complete_method(&lines, line_number);

        if method_lines.len() > 1 {
            return Ok(method_lines.join("\n"));
        }

        let start_line = if line_number > context_lines {
            line_number - context_lines
        } else {
            1
        };
        let end_line = std::cmp::min(line_number + context_lines, lines.len());

        let mut result = String::new();
        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            if line_num >= start_line && line_num <= end_line {
                let marker = if line_num == line_number {
                    ">>>"
                } else {
                    "   "
                };
                result.push_str(&format!("{marker} {line_num:4}: {line}\n"));
            }
        }

        Ok(result)
    }

    pub async fn find_and_display_definition(
        &self,
        symbol: &str,
        path_filter: Option<&str>,
        options: &SearchOptions,
    ) -> Result<String> {
        debug!("Finding potential definition locations...");
        let file_locations = self
            .find_symbol_locations(symbol, path_filter, options)
            .await?;

        if file_locations.is_empty() {
            error!("No potential definitions found for '{symbol}'");
            return Ok(String::new());
        }

        debug!(
            "Found {} potential definition location(s)",
            file_locations.len()
        );

        if let Some((file_path, line_number)) = file_locations.first() {
            match self
                .get_definition_context(file_path, *line_number, 10, Some(symbol))
                .await
            {
                Ok(context) => Ok(context),
                Err(e) => {
                    error!("Could not fetch context: {e}");
                    Ok(String::new())
                }
            }
        } else {
            error!("No definition found for symbol '{symbol}'");
            Ok(String::new())
        }
    }
}
