use crate::client::SearchfoxClient;
use crate::types::SearchfoxResponse;
use anyhow::Result;
use reqwest::Url;
use serde_json;

pub struct CallGraphQuery {
    pub calls_from: Option<String>,
    pub calls_to: Option<String>,
    pub calls_between: Option<(String, String)>,
    pub depth: u32,
}

impl SearchfoxClient {
    pub async fn search_call_graph(&self, query: &CallGraphQuery) -> Result<serde_json::Value> {
        let query_string = if let Some(symbol) = &query.calls_from {
            format!(
                "calls-from:'{}' depth:{} graph-format:json",
                symbol, query.depth
            )
        } else if let Some(symbol) = &query.calls_to {
            format!(
                "calls-to:'{}' depth:{} graph-format:json",
                symbol, query.depth
            )
        } else if let Some((source, target)) = &query.calls_between {
            format!(
                "calls-between-source:'{}' calls-between-target:'{}' depth:{} graph-format:json",
                source.trim(),
                target.trim(),
                query.depth
            )
        } else {
            anyhow::bail!("No call graph query specified");
        };

        let mut url = Url::parse(&format!(
            "https://searchfox.org/{}/query/default",
            self.repo
        ))?;
        url.query_pairs_mut().append_pair("q", &query_string);

        let response = self.get(url).await?;

        if !response.status().is_success() {
            anyhow::bail!("Request failed: {}", response.status());
        }

        let response_text = response.text().await?;

        match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(json) => {
                if let Some(symbol_graph) = json.get("SymbolGraphCollection") {
                    Ok(symbol_graph.clone())
                } else {
                    match serde_json::from_str::<SearchfoxResponse>(&response_text) {
                        Ok(parsed_json) => {
                            let mut result = serde_json::json!({});
                            for (key, value) in &parsed_json {
                                if !key.starts_with('*') {
                                    if value.as_array().is_some() || value.as_object().is_some() {
                                        result[key] = value.clone();
                                    }
                                }
                            }
                            Ok(result)
                        }
                        Err(_) => Ok(json),
                    }
                }
            }
            Err(_) => Ok(serde_json::json!({
                "error": "Failed to parse response as JSON",
                "raw_response": response_text
            })),
        }
    }
}