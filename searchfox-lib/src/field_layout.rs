use crate::client::SearchfoxClient;
use crate::types::SearchfoxResponse;
use anyhow::Result;
use reqwest::Url;
use serde_json;
use tabled::{
    settings::{object::Rows, Color, Modify, Style},
    Table, Tabled,
};

pub struct FieldLayoutQuery {
    pub class_name: String,
}

#[derive(Tabled)]
struct BaseClass {
    offset: u64,
    size: u64,
    #[tabled(rename = "type")]
    base_type: String,
}

#[derive(Tabled)]
struct Field {
    offset: u64,
    size: u64,
    #[tabled(rename = "type")]
    field_type: String,
    name: String,
}

fn wrap_cpp_type(type_str: &str, max_width: usize) -> String {
    if type_str.len() <= max_width {
        return type_str.to_string();
    }

    let mut result = String::new();
    let mut current_line = String::new();
    let mut depth = 0;
    let mut i = 0;
    let chars: Vec<char> = type_str.chars().collect();

    while i < chars.len() {
        let ch = chars[i];

        match ch {
            '<' => {
                current_line.push(ch);
                depth += 1;
                if current_line.len() > max_width && depth == 1 {
                    result.push_str(&current_line);
                    result.push('\n');
                    current_line.clear();
                    current_line.push_str(&"  ".repeat(depth));
                }
            }
            '>' => {
                current_line.push(ch);
                depth = depth.saturating_sub(1);
            }
            ',' => {
                current_line.push(ch);
                if i + 1 < chars.len() && chars[i + 1] == ' ' {
                    i += 1;
                }
                if depth > 0 && current_line.len() > max_width / 2 {
                    result.push_str(current_line.trim_end());
                    result.push('\n');
                    current_line.clear();
                    current_line.push_str(&"  ".repeat(depth));
                } else {
                    current_line.push(' ');
                }
            }
            _ => {
                current_line.push(ch);
            }
        }

        if current_line.len() > max_width && !current_line.trim().is_empty() && depth > 0 {
            if let Some(last_space) = current_line.rfind(' ') {
                if last_space > max_width / 2 {
                    let (left, right) = current_line.split_at(last_space);
                    result.push_str(left.trim_end());
                    result.push('\n');
                    current_line = format!("{}{}", "  ".repeat(depth), right.trim_start());
                }
            }
        }

        i += 1;
    }

    if !current_line.is_empty() {
        result.push_str(&current_line);
    }

    result
}

pub fn format_field_layout(class_name: &str, json: &serde_json::Value) -> String {
    let mut output = String::new();
    output.push_str(&format!("Field Layout: {}\n\n", class_name));

    let terminal_width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(100);

    let type_col_max_width = (terminal_width.saturating_sub(40)).clamp(30, 60);

    let symbol_key = format!("T_{}", class_name);

    let mut found = false;

    if let Some(tables) = json
        .get("SymbolTreeTableList")
        .and_then(|v| v.get("tables"))
        .and_then(|v| v.as_array())
    {
        for table in tables {
            if let Some(jumprefs) = table.get("jumprefs").and_then(|v| v.as_object()) {
                if let Some(symbol_info) = jumprefs.get(&symbol_key) {
                    found = true;

                    let meta = if let Some(variants) = symbol_info
                        .get("meta")
                        .and_then(|m| m.get("variants"))
                        .and_then(|v| v.as_array())
                    {
                        variants.first()
                    } else {
                        symbol_info.get("meta")
                    };

                    if let Some(meta_obj) = meta {
                        if let Some(size) = meta_obj.get("sizeBytes").and_then(|v| v.as_u64()) {
                            output.push_str(&format!("Size: {} bytes", size));
                        }

                        if let Some(alignment) =
                            meta_obj.get("alignmentBytes").and_then(|v| v.as_u64())
                        {
                            output.push_str(&format!(", Alignment: {} bytes\n\n", alignment));
                        } else {
                            output.push_str("\n\n");
                        }

                        if let Some(supers) = meta_obj.get("supers").and_then(|v| v.as_array()) {
                            if !supers.is_empty() {
                                let mut base_classes = Vec::new();

                                for base in supers {
                                    if let Some(base_obj) = base.as_object() {
                                        let offset = base_obj
                                            .get("offsetBytes")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let size = base_obj
                                            .get("sizeBytes")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let base_sym = base_obj
                                            .get("sym")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        let base_type =
                                            base_sym.strip_prefix("T_").unwrap_or(base_sym);
                                        let wrapped_type =
                                            wrap_cpp_type(base_type, type_col_max_width);

                                        base_classes.push(BaseClass {
                                            offset,
                                            size,
                                            base_type: wrapped_type,
                                        });
                                    }
                                }

                                let mut table = Table::new(&base_classes);
                                table
                                    .with(Style::rounded())
                                    .with(Modify::new(Rows::first()).with(Color::FG_GREEN));

                                output.push_str("Base Classes:\n");
                                output.push_str(&format!("{}\n\n", table));
                            }
                        }

                        if let Some(fields) = meta_obj.get("fields").and_then(|v| v.as_array()) {
                            if !fields.is_empty() {
                                let mut field_list = Vec::new();

                                for field in fields {
                                    if let Some(field_obj) = field.as_object() {
                                        let offset = field_obj
                                            .get("offsetBytes")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let size = field_obj
                                            .get("sizeBytes")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let field_type = field_obj
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        let name = field_obj
                                            .get("pretty")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| s.split("::").last())
                                            .unwrap_or("unnamed");
                                        let wrapped_type =
                                            wrap_cpp_type(field_type, type_col_max_width);

                                        field_list.push(Field {
                                            offset,
                                            size,
                                            field_type: wrapped_type,
                                            name: name.to_string(),
                                        });
                                    }
                                }

                                let mut table = Table::new(&field_list);
                                table
                                    .with(Style::rounded())
                                    .with(Modify::new(Rows::first()).with(Color::FG_CYAN));

                                output.push_str("Fields:\n");
                                output.push_str(&format!("{}\n", table));
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    if !found {
        output.push_str("No field layout information found.\n");
        output.push_str("This feature only works with C++ classes and structs.\n");
    }

    output
}

impl SearchfoxClient {
    pub async fn search_field_layout(&self, query: &FieldLayoutQuery) -> Result<serde_json::Value> {
        let query_string = format!("field-layout:'{}'", query.class_name);

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
                if let Some(_symbol_tree) = json.get("SymbolTreeTableList") {
                    Ok(json)
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
