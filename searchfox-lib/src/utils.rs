use crate::types::Line;

pub fn is_mozilla_repository() -> bool {
    std::path::Path::new("./mach").exists()
}

pub fn read_local_file(file_path: &str) -> Option<String> {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        return Some(content);
    }
    if let Ok(content) = std::fs::read_to_string(format!("./{file_path}")) {
        return Some(content);
    }
    None
}

pub fn find_symbol_in_local_content(
    content: &str,
    expected_line: usize,
    symbol: &str,
) -> Option<usize> {
    let lines: Vec<&str> = content.lines().collect();

    if expected_line > 0 && expected_line <= lines.len() {
        let line_idx = expected_line - 1;
        if lines[line_idx].contains(symbol)
            || (symbol.contains("::")
                && lines[line_idx].contains(symbol.split("::").last().unwrap_or("")))
        {
            return Some(expected_line);
        }
    }

    let search_range = 50;
    let start = expected_line.saturating_sub(search_range);
    let end = std::cmp::min(expected_line + search_range, lines.len());

    for i in start..end {
        if i < lines.len() {
            let line = lines[i];
            if (line.contains(symbol)
                || (symbol.contains("::")
                    && line.contains(symbol.split("::").last().unwrap_or(""))))
                && (line.contains("::") || line.contains("(") || line.contains("="))
            {
                return Some(i + 1);
            }
        }
    }

    None
}

pub fn extract_complete_method(lines: &[&str], start_line: usize) -> (usize, Vec<String>) {
    let start_idx = start_line.saturating_sub(1);
    if start_idx >= lines.len() {
        return (
            start_line,
            vec![lines.get(start_idx).unwrap_or(&"").to_string()],
        );
    }

    let start_line_content = lines[start_idx];

    let looks_like_function = (start_line_content.contains("(")
        && (start_line_content.contains("{")
            || start_line_content.trim_end().ends_with(")")
            || start_line_content.trim_end().ends_with(";")
            || start_line_content.contains("::")
            || start_line_content.trim_start().starts_with("fn ")
            || start_line_content.contains("function ")))
        || start_line_content.contains("class ")
        || start_line_content.contains("struct ")
        || start_line_content.contains("interface ");

    if !looks_like_function {
        let mut found_function_pattern = false;
        for i in 0..=5.min(lines.len().saturating_sub(start_idx + 1)) {
            if let Some(line) = lines.get(start_idx + i) {
                if line.contains("{") || line.trim_start().starts_with(":") {
                    found_function_pattern = true;
                    break;
                }
            }
        }

        if !found_function_pattern {
            let context_start = start_idx.saturating_sub(5);
            let context_end = std::cmp::min(start_idx + 5, lines.len());
            let context_lines: Vec<String> = lines[context_start..context_end]
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let line_num = context_start + i + 1;
                    let marker = if line_num == start_line { ">>>" } else { "   " };
                    format!("{marker} {line_num:4}: {line}")
                })
                .collect();
            return (start_line, context_lines);
        }
    }

    if start_line_content.trim_end().ends_with(';')
        && !(start_line_content.contains("class ") || start_line_content.contains("struct "))
    {
        return (
            start_line,
            vec![format!(">>> {:4}: {}", start_line, start_line_content)],
        );
    }

    let mut found_opening_brace = start_line_content.contains('{');

    if !found_opening_brace {
        for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
            if line.contains('{') {
                found_opening_brace = true;
                break;
            }
            if i > start_idx + 25
                || (line.trim().is_empty() && i > start_idx + 5)
                || (line.contains("::")
                    && line.contains("(")
                    && !line.trim_start().starts_with("//")
                    && !line.contains("mId")
                    && !line.contains("m"))
            {
                break;
            }
        }
    }

    if !found_opening_brace {
        return (
            start_line,
            vec![format!(">>> {:4}: {}", start_line, start_line_content)],
        );
    }

    let mut result_lines = Vec::new();
    let mut brace_count = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut in_single_comment = false;
    let mut in_multi_comment = false;

    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        let line_num = i + 1;
        let marker = if line_num == start_line { ">>>" } else { "   " };
        result_lines.push(format!("{marker} {line_num:4}: {line}"));

        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            let ch = chars[j];
            let next_ch = chars.get(j + 1).copied();

            if escaped {
                escaped = false;
                j += 1;
                continue;
            }

            match ch {
                '\\' if in_string || in_char => escaped = true,
                '"' if !in_char && !in_single_comment && !in_multi_comment => {
                    in_string = !in_string
                }
                '\'' if !in_string && !in_single_comment && !in_multi_comment => in_char = !in_char,
                '/' if !in_string && !in_char && !in_single_comment && !in_multi_comment => {
                    if next_ch == Some('/') {
                        in_single_comment = true;
                        j += 1;
                    } else if next_ch == Some('*') {
                        in_multi_comment = true;
                        j += 1;
                    }
                }
                '*' if in_multi_comment && next_ch == Some('/') => {
                    in_multi_comment = false;
                    j += 1;
                }
                '{' if !in_string && !in_char && !in_single_comment && !in_multi_comment => {
                    brace_count += 1;
                }
                '}' if !in_string && !in_char && !in_single_comment && !in_multi_comment => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        let is_class_or_struct = lines[start_idx].contains("class ")
                            || lines[start_idx].contains("struct ");
                        if is_class_or_struct {
                            let remaining_on_line = &line[j + 1..];
                            if remaining_on_line.trim().starts_with(';') {
                                return (start_line, result_lines);
                            } else if i + 1 < lines.len() {
                                let next_line = lines[i + 1];
                                if next_line.trim().starts_with(';') {
                                    result_lines.push(format!("     {:4}: {}", i + 2, next_line));
                                }
                            }
                        }
                        return (start_line, result_lines);
                    }
                }
                _ => {}
            }
            j += 1;
        }

        in_single_comment = false;

        if result_lines.len() > 200 {
            result_lines.push("   ...  : (method too long, truncated)".to_string());
            break;
        }
    }

    (start_line, result_lines)
}

pub fn is_potential_definition(line: &Line, query: &str) -> bool {
    let line_text = &line.line;
    let line_lower = line_text.to_lowercase();
    let query_lower = query.to_lowercase();

    let contains_query = line_text.contains(query) || line_lower.contains(&query_lower);

    if contains_query {
        let looks_like_definition = line_text.contains("{")
            || line_text.trim_end().ends_with(';')
            || line_text.contains("=")
            || line_text.contains("class ")
            || line_text.contains("struct ")
            || line_text.contains("interface ")
            || (line_text.contains("::")
                && (line_text.contains("(")
                    || line_text.contains("already_AddRefed")
                    || line_text.contains("RefPtr")
                    || line_text.contains("nsCOMPtr")));

        looks_like_definition
    } else {
        false
    }
}

pub fn get_github_raw_url(repo: &str, file_path: &str) -> String {
    let github_repo = match repo {
        "comm-central" => "mozilla/releases-comm-central",
        _ => "mozilla/firefox",
    };

    let branch = match repo {
        "mozilla-central" => "main",
        "autoland" => "autoland",
        "mozilla-beta" => "beta",
        "mozilla-release" => "release",
        "mozilla-esr115" => "esr115",
        "mozilla-esr128" => "esr128",
        "mozilla-esr140" => "esr140",
        "comm-central" => "main",
        _ => "main",
    };

    format!("https://raw.githubusercontent.com/{github_repo}/{branch}/{file_path}")
}
