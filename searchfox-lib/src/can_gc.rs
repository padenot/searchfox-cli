use crate::call_graph::CallGraphQuery;
use crate::client::SearchfoxClient;
use anyhow::Result;
use reqwest::Url;
use std::collections::HashSet;

pub struct GcInfo {
    pub pretty: String,
    pub mangled: String,
    pub can_gc: bool,
    pub gc_path: Option<String>,
}

impl SearchfoxClient {
    pub async fn get_gc_info(&self, symbol: &str) -> Result<Vec<GcInfo>> {
        let results = self.query_gc_from_call_graph(symbol).await?;
        if !results.is_empty() {
            return Ok(results);
        }

        // The partial name didn't work with the call graph — resolve to fully-qualified
        // names via the id: search endpoint and retry.
        let full_names = self.resolve_full_names(symbol).await?;
        let mut all_results = Vec::new();
        let mut seen_keys: HashSet<(String, bool, Option<String>)> = HashSet::new();

        for full_name in full_names {
            if full_name == symbol {
                continue;
            }
            for info in self.query_gc_from_call_graph(&full_name).await? {
                let key = (info.pretty.clone(), info.can_gc, info.gc_path.clone());
                if seen_keys.insert(key) {
                    all_results.push(info);
                }
            }
        }

        Ok(all_results)
    }

    async fn query_gc_from_call_graph(&self, symbol: &str) -> Result<Vec<GcInfo>> {
        let query = CallGraphQuery {
            calls_from: Some(symbol.to_string()),
            calls_to: None,
            calls_between: None,
            depth: 1,
        };

        let result = self.search_call_graph(&query).await?;

        let mut results = Vec::new();
        let mut seen: HashSet<(String, bool, Option<String>)> = HashSet::new();

        if let Some(jumprefs) = result.get("jumprefs").and_then(|v| v.as_object()) {
            for (mangled, info) in jumprefs {
                let pretty = match info.get("pretty").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => continue,
                };

                if !symbol_matches(pretty, symbol) {
                    continue;
                }

                let meta = match info.get("meta") {
                    Some(m) => m,
                    None => continue,
                };

                let can_gc = match meta.get("canGC").and_then(|v| v.as_bool()) {
                    Some(v) => v,
                    None => continue,
                };

                let gc_path = meta
                    .get("gcPath")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let key = (pretty.to_string(), can_gc, gc_path.clone());
                if seen.insert(key) {
                    results.push(GcInfo {
                        pretty: pretty.to_string(),
                        mangled: mangled.clone(),
                        can_gc,
                        gc_path,
                    });
                }
            }
        }

        Ok(results)
    }

    async fn resolve_full_names(&self, symbol: &str) -> Result<Vec<String>> {
        let query = format!("id:{symbol}");
        let mut url = Url::parse(&format!("https://searchfox.org/{}/search", self.repo))?;
        url.query_pairs_mut().append_pair("q", &query);

        let response = self.get(url).await?;
        if !response.status().is_success() {
            return Ok(vec![]);
        }

        let text = response.text().await?;
        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };

        let top = match json.as_object() {
            Some(o) => o,
            None => return Ok(vec![]),
        };

        let symbol_suffix = if let Some(pos) = symbol.rfind("::") {
            &symbol[pos + 2..]
        } else {
            symbol
        };

        let mut names = Vec::new();

        // The response has top-level keys like "normal"/"generated" whose values are
        // objects mapping category names like "Definitions (full::Name)" to file arrays.
        for (top_key, top_val) in top {
            if top_key.starts_with('*') {
                continue;
            }
            let categories = match top_val.as_object() {
                Some(o) => o,
                None => continue,
            };
            for cat_key in categories.keys() {
                if (cat_key.contains("Definitions") || cat_key.contains("Declarations"))
                    && cat_key.ends_with(')')
                {
                    if let Some(open) = cat_key.rfind('(') {
                        let name = &cat_key[open + 1..cat_key.len() - 1];
                        if name == symbol
                            || name.ends_with(&format!("::{symbol}"))
                            || name.ends_with(&format!("::{symbol_suffix}"))
                        {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }

        names.sort();
        names.dedup();
        Ok(names)
    }
}

fn symbol_matches(pretty: &str, query: &str) -> bool {
    pretty == query
        || pretty.ends_with(&format!("::{query}"))
        || pretty.starts_with(&format!("{query}("))
        || pretty.starts_with(&format!("{query}<"))
        || pretty.contains(&format!("::{query}("))
        || pretty.contains(&format!("::{query}<"))
}
