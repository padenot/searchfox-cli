use crate::client::SearchfoxClient;
use crate::types::{File, SearchfoxResponse};
use anyhow::Result;
use log::{debug, warn};
use reqwest::Url;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub query: Option<String>,
    pub path: Option<String>,
    pub case: bool,
    pub regexp: bool,
    pub limit: usize,
    pub context: Option<usize>,
    pub symbol: Option<String>,
    pub id: Option<String>,
    pub cpp: bool,
    pub c_lang: bool,
    pub webidl: bool,
    pub js: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: None,
            path: None,
            case: false,
            regexp: false,
            limit: 50,
            context: None,
            symbol: None,
            id: None,
            cpp: false,
            c_lang: false,
            webidl: false,
            js: false,
        }
    }
}

impl SearchOptions {
    /// Check if this search is expensive (doesn't use searchfox's index)
    pub fn is_expensive_search(&self) -> bool {
        // Only symbol: and id: prefixes use the optimized index
        if self.symbol.is_some() || self.id.is_some() {
            return false;
        }

        if let Some(query) = &self.query {
            // Check if query contains indexed prefixes
            !query.contains("symbol:") && !query.contains("id:")
        } else {
            false
        }
    }

    pub fn matches_language_filter(&self, path: &str) -> bool {
        if !self.cpp && !self.c_lang && !self.webidl && !self.js {
            return true;
        }

        let path_lower = path.to_lowercase();

        if self.cpp
            && (path_lower.ends_with(".cc")
                || path_lower.ends_with(".cpp")
                || path_lower.ends_with(".h")
                || path_lower.ends_with(".hh")
                || path_lower.ends_with(".hpp"))
        {
            return true;
        }

        if self.c_lang && (path_lower.ends_with(".c") || path_lower.ends_with(".h")) {
            return true;
        }

        if self.webidl && path_lower.ends_with(".webidl") {
            return true;
        }

        if self.js
            && (path_lower.ends_with(".js")
                || path_lower.ends_with(".mjs")
                || path_lower.ends_with(".ts")
                || path_lower.ends_with(".cjs")
                || path_lower.ends_with(".jsx")
                || path_lower.ends_with(".tsx"))
        {
            return true;
        }

        false
    }

    pub fn build_query(&self) -> String {
        if let Some(symbol) = &self.symbol {
            format!("symbol:{symbol}")
        } else if let Some(id) = &self.id {
            format!("id:{id}")
        } else if let Some(q) = &self.query {
            if q.contains("path:")
                || q.contains("pathre:")
                || q.contains("symbol:")
                || q.contains("id:")
                || q.contains("text:")
                || q.contains("re:")
            {
                q.clone()
            } else if let Some(context) = self.context {
                format!("context:{context} text:{q}")
            } else {
                q.clone()
            }
        } else {
            String::new()
        }
    }
}

pub struct SearchResult {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

impl SearchfoxClient {
    /// Warns about expensive searches to stderr (for library users)
    pub fn warn_if_expensive_search(&self, options: &SearchOptions) {
        if options.is_expensive_search() {
            if let Some(query) = &options.query {
                eprintln!("⚠️  WARNING: Expensive full-text search detected");
                eprintln!("Query '{}' doesn't use searchfox's optimized index", query);
                eprintln!("Consider using symbol: or id: prefixes, or use ripgrep locally");
                eprintln!("For LLM tools: Use find_and_display_definition() for definitions");
            }
        }
    }

    pub async fn search(&self, options: &SearchOptions) -> Result<Vec<SearchResult>> {
        let query = options.build_query();

        let mut url = Url::parse(&format!("https://searchfox.org/{}/search", self.repo))?;
        url.query_pairs_mut()
            .append_pair("q", &query)
            .append_pair("case", if options.case { "true" } else { "false" })
            .append_pair("regexp", if options.regexp { "true" } else { "false" });
        if let Some(path) = &options.path {
            url.query_pairs_mut().append_pair("path", path);
        }

        let response = self.get(url).await?;

        if !response.status().is_success() {
            anyhow::bail!("Request failed: {}", response.status());
        }

        let response_text = response.text().await?;
        let json: SearchfoxResponse = serde_json::from_str(&response_text)?;

        let mut results = Vec::new();
        let mut count = 0;

        for (key, value) in &json {
            if key.starts_with('*') {
                continue;
            }

            if let Some(files_array) = value.as_array() {
                for file in files_array {
                    let file: File = match serde_json::from_value(file.clone()) {
                        Ok(f) => f,
                        Err(e) => {
                            warn!("Failed to parse file JSON: {e}");
                            continue;
                        }
                    };

                    if !options.matches_language_filter(&file.path) {
                        continue;
                    }

                    if options.path.is_some()
                        && options.query.is_none()
                        && options.symbol.is_none()
                        && options.id.is_none()
                    {
                        if count >= options.limit {
                            break;
                        }
                        results.push(SearchResult {
                            path: file.path.clone(),
                            line_number: 0,
                            line: String::new(),
                        });
                        count += 1;
                    } else {
                        for line in file.lines {
                            if count >= options.limit {
                                break;
                            }
                            results.push(SearchResult {
                                path: file.path.clone(),
                                line_number: line.lno,
                                line: line.line.trim_end().to_string(),
                            });
                            count += 1;
                        }
                    }
                }
            } else if let Some(obj) = value.as_object() {
                for (_category, file_list) in obj {
                    if let Some(files) = file_list.as_array() {
                        for file in files {
                            let file: File = match serde_json::from_value(file.clone()) {
                                Ok(f) => f,
                                Err(_) => continue,
                            };

                            if !options.matches_language_filter(&file.path) {
                                continue;
                            }

                            if options.path.is_some()
                                && options.query.is_none()
                                && options.symbol.is_none()
                                && options.id.is_none()
                            {
                                if count >= options.limit {
                                    break;
                                }
                                results.push(SearchResult {
                                    path: file.path.clone(),
                                    line_number: 0,
                                    line: String::new(),
                                });
                                count += 1;
                            } else {
                                for line in file.lines {
                                    if count >= options.limit {
                                        break;
                                    }
                                    results.push(SearchResult {
                                        path: file.path.clone(),
                                        line_number: line.lno,
                                        line: line.line.trim_end().to_string(),
                                    });
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }

            if count >= options.limit {
                break;
            }
        }

        Ok(results)
    }

    pub async fn find_symbol_locations(
        &self,
        symbol: &str,
        path_filter: Option<&str>,
        options: &SearchOptions,
    ) -> Result<Vec<(String, usize)>> {
        let query = format!("id:{symbol}");
        let mut url = Url::parse(&format!("https://searchfox.org/{}/search", self.repo))?;
        url.query_pairs_mut().append_pair("q", &query);
        if let Some(path) = path_filter {
            url.query_pairs_mut().append_pair("path", path);
        }

        let response = self.get(url).await?;

        if !response.status().is_success() {
            anyhow::bail!("Request failed: {}", response.status());
        }

        let response_text = response.text().await?;
        let json: SearchfoxResponse = serde_json::from_str(&response_text)?;
        let mut file_locations = Vec::new();

        debug!("Analyzing search results...");

        for (key, value) in &json {
            if key.starts_with('*') {
                continue;
            }

            if let Some(files_array) = value.as_array() {
                debug!("Found {} files in array for key {}", files_array.len(), key);
                for file in files_array {
                    match serde_json::from_value::<File>(file.clone()) {
                        Ok(file) => {
                            if !options.matches_language_filter(&file.path) {
                                continue;
                            }

                            debug!(
                                "Processing file: {} with {} lines",
                                file.path,
                                file.lines.len()
                            );
                            for line in file.lines {
                                if crate::utils::is_potential_definition(&line, symbol) {
                                    debug!(
                                        "Found potential definition: {}:{} - {}",
                                        file.path,
                                        line.lno,
                                        line.line.trim()
                                    );
                                    file_locations.push((file.path.clone(), line.lno));
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse file JSON: {e}");
                        }
                    }
                }
            } else if let Some(categories) = value.as_object() {
                let symbol_name = symbol.strip_prefix("id:").unwrap_or(symbol);
                let is_method_search = symbol_name.contains("::");

                if !is_method_search {
                    let class_def_key = format!("Definitions ({symbol_name})");
                    if let Some(files_array) =
                        categories.get(&class_def_key).and_then(|v| v.as_array())
                    {
                        for file in files_array {
                            match serde_json::from_value::<File>(file.clone()) {
                                Ok(file) => {
                                    if !options.matches_language_filter(&file.path) {
                                        continue;
                                    }

                                    for line in file.lines {
                                        if line.line.contains("class ")
                                            || line.line.contains("struct ")
                                        {
                                            debug!(
                                                "Found class/struct definition: {}:{} - {}",
                                                file.path,
                                                line.lno,
                                                line.line.trim()
                                            );
                                            file_locations.push((file.path.clone(), line.lno));
                                        }
                                    }
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                }

                let search_order = if is_method_search {
                    vec!["Definitions", "Declarations"]
                } else {
                    vec!["Declarations", "Definitions"]
                };

                for search_type in search_order {
                    for (category_name, category_value) in categories {
                        if !is_method_search {
                            let class_def_key = format!("Definitions ({symbol_name})");
                            if category_name == &class_def_key {
                                continue;
                            }
                        }

                        if category_name.contains(search_type)
                            && (category_name.contains(symbol_name)
                                || category_name
                                    .to_lowercase()
                                    .contains(&symbol_name.to_lowercase()))
                        {
                            if let Some(files_array) = category_value.as_array() {
                                for file in files_array {
                                    match serde_json::from_value::<File>(file.clone()) {
                                        Ok(file) => {
                                            if !options.matches_language_filter(&file.path) {
                                                continue;
                                            }

                                            for line in file.lines {
                                                if let Some(upsearch) = &line.upsearch {
                                                    if upsearch.starts_with("symbol:_Z") {
                                                        return Ok(vec![(
                                                            file.path.clone(),
                                                            line.lno,
                                                        )]);
                                                    }
                                                }
                                                file_locations.push((file.path.clone(), line.lno));
                                            }
                                        }
                                        Err(_) => continue,
                                    }
                                }
                            }
                        }
                    }

                    if !file_locations.is_empty() {
                        break;
                    }
                }
            }
        }

        Ok(file_locations)
    }
}
