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

pub fn format_call_graph_markdown(query_text: &str, json: &serde_json::Value) -> String {
    use std::collections::{BTreeMap, BTreeSet};

    let mut output = String::new();
    output.push_str(&format!("# {}\n\n", query_text));

    let is_calls_between = query_text.contains("calls-between");

    if is_calls_between {
        if let Some(hierarchical_graphs) = json.get("hierarchicalGraphs").and_then(|v| v.as_array()) {
            let jumprefs = json.get("jumprefs").and_then(|v| v.as_object());

            let mut all_edges = Vec::new();

            fn collect_edges(node: &serde_json::Value, edges: &mut Vec<(String, String)>) {
                if let Some(node_edges) = node.get("edges").and_then(|e| e.as_array()) {
                    for edge in node_edges {
                        if let Some(edge_obj) = edge.as_object() {
                            let from = edge_obj.get("from").and_then(|f| f.as_str()).unwrap_or("");
                            let to = edge_obj.get("to").and_then(|t| t.as_str()).unwrap_or("");
                            if !from.is_empty() && !to.is_empty() {
                                edges.push((from.to_string(), to.to_string()));
                            }
                        }
                    }
                }

                if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                    for child in children {
                        collect_edges(child, edges);
                    }
                }
            }

            for hg in hierarchical_graphs {
                collect_edges(hg, &mut all_edges);
            }

            if all_edges.is_empty() {
                output.push_str("No direct calls found between source and target.\n");
            } else {
                output.push_str("## Direct calls from source to target\n\n");

                for (from_sym, to_sym) in all_edges {
                    let from_pretty = if let Some(jumprefs) = jumprefs {
                        jumprefs.get(&from_sym)
                            .and_then(|s| s.get("pretty"))
                            .and_then(|p| p.as_str())
                            .unwrap_or(&from_sym)
                    } else {
                        &from_sym
                    };

                    let from_location = if let Some(jumprefs) = jumprefs {
                        jumprefs.get(&from_sym)
                            .and_then(|s| s.get("jumps"))
                            .and_then(|j| j.get("def"))
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                    } else {
                        ""
                    };

                    let to_pretty = if let Some(jumprefs) = jumprefs {
                        jumprefs.get(&to_sym)
                            .and_then(|s| s.get("pretty"))
                            .and_then(|p| p.as_str())
                            .unwrap_or(&to_sym)
                    } else {
                        &to_sym
                    };

                    let to_location = if let Some(jumprefs) = jumprefs {
                        jumprefs.get(&to_sym)
                            .and_then(|s| s.get("jumps"))
                            .and_then(|j| j.get("def"))
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                    } else {
                        ""
                    };

                    output.push_str(&format!("- **{}** ({}) calls **{}** ({})\n",
                        from_pretty, from_location, to_pretty, to_location));
                    output.push_str(&format!("  - From: `{}`\n", from_sym));
                    output.push_str(&format!("  - To: `{}`\n", to_sym));
                }
            }

            return output;
        }
    }

    let mut grouped_by_parent: BTreeMap<String, BTreeSet<(String, String, String, String)>> = BTreeMap::new();

    let jumprefs = json.get("jumprefs").and_then(|v| v.as_object());

    let is_calls_to = query_text.contains("calls-to:");

    if let Some(graphs) = json.get("graphs").and_then(|v| v.as_array()) {
        for graph in graphs {
            if let Some(edges) = graph.get("edges").and_then(|v| v.as_array()) {
                for edge in edges {
                    if let Some(edge_obj) = edge.as_object() {
                        let target_sym = if is_calls_to {
                            edge_obj.get("from").and_then(|v| v.as_str()).unwrap_or("")
                        } else {
                            edge_obj.get("to").and_then(|v| v.as_str()).unwrap_or("")
                        };

                        if let Some(jumprefs) = jumprefs {
                            if let Some(symbol_info) = jumprefs.get(target_sym) {
                                let pretty_name = symbol_info
                                    .get("pretty")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let mangled = symbol_info
                                    .get("sym")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(target_sym);

                                let jumps = symbol_info.get("jumps");

                                let decl_location = jumps
                                    .and_then(|j| j.get("decl"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                let def_location = jumps
                                    .and_then(|j| j.get("def"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                let location = if !def_location.is_empty() && !decl_location.is_empty() && def_location != decl_location {
                                    format!("{} (decl: {})", def_location, decl_location)
                                } else if !def_location.is_empty() {
                                    def_location.to_string()
                                } else if !decl_location.is_empty() {
                                    decl_location.to_string()
                                } else {
                                    String::new()
                                };

                                let parent_sym = symbol_info
                                    .get("meta")
                                    .and_then(|m| m.get("parentsym"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Free functions");

                                let parent_sym_clean = if parent_sym.starts_with("T_") {
                                    &parent_sym[2..]
                                } else if parent_sym == "Free functions" {
                                    parent_sym
                                } else {
                                    parent_sym
                                };

                                if !pretty_name.is_empty() && !location.is_empty() {
                                    grouped_by_parent
                                        .entry(parent_sym_clean.to_string())
                                        .or_insert_with(BTreeSet::new)
                                        .insert((
                                            pretty_name.to_string(),
                                            mangled.to_string(),
                                            location,
                                            String::new(),
                                        ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for (parent_sym, items) in grouped_by_parent {
        output.push_str(&format!("## {}\n\n", parent_sym));

        let mut grouped_items: Vec<(String, Vec<(String, String)>)> = Vec::new();

        for (pretty_name, mangled, location, _) in items {
            if let Some((last_pretty, last_overloads)) = grouped_items.last_mut() {
                if last_pretty == &pretty_name {
                    last_overloads.push((mangled, location));
                    continue;
                }
            }
            grouped_items.push((pretty_name, vec![(mangled, location)]));
        }

        for (pretty_name, overloads) in grouped_items {
            if overloads.len() == 1 {
                let (mangled, location) = &overloads[0];
                output.push_str(&format!("- {} (`{}`, {})\n", pretty_name, mangled, location));
            } else {
                output.push_str(&format!("- {} ({} overloads)\n", pretty_name, overloads.len()));
                for (mangled, location) in overloads {
                    output.push_str(&format!("  - `{}`, {}\n", mangled, location));
                }
            }
        }
        output.push('\n');
    }

    output
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
                                if !key.starts_with('*')
                                    && (value.as_array().is_some() || value.as_object().is_some())
                                {
                                    result[key] = value.clone();
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
