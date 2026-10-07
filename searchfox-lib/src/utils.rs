use crate::types::Line;

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

/// Heuristically checks for `= 0;` within 25 lines of the 1-based `start_line`.
pub fn is_pure_virtual_declaration(lines: &[&str], start_line: usize) -> bool {
    let start_idx = start_line.saturating_sub(1);
    let mut paren_depth = 0usize;
    // Exclude default arguments such as `int flags = 0`.
    let mut top_level = String::new();
    let mut in_multi_comment = false;

    for line in lines.iter().skip(start_idx).take(25) {
        let chars: Vec<char> = line.chars().collect();
        // Opening quote, if inside a literal.
        let mut literal = None;
        let mut j = 0;
        while j < chars.len() {
            let ch = chars[j];
            let next_ch = chars.get(j + 1).copied();

            if in_multi_comment {
                if ch == '*' && next_ch == Some('/') {
                    in_multi_comment = false;
                    j += 1;
                }
                j += 1;
                continue;
            }

            if let Some(quote) = literal {
                if ch == '\\' {
                    j += 1;
                } else if ch == quote {
                    literal = None;
                }
                j += 1;
                continue;
            }

            match ch {
                '"' => literal = Some('"'),
                '\'' if !is_digit_separator(&chars, j) => literal = Some('\''),
                '/' if next_ch == Some('/') => break,
                '/' if next_ch == Some('*') => {
                    in_multi_comment = true;
                    j += 1;
                }
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '{' if paren_depth == 0 => return false,
                ';' if paren_depth == 0 => {
                    let code: String = top_level.chars().filter(|c| !c.is_whitespace()).collect();
                    return code.ends_with("=0");
                }
                _ if paren_depth == 0 => top_level.push(ch),
                _ => {}
            }
            j += 1;
        }
    }

    false
}

/// Treats a quote as a digit separator if the preceding token starts with a
/// digit, ignoring leading dots (e.g. `.5'5`).
fn is_digit_separator(chars: &[char], quote_idx: usize) -> bool {
    let token_start = chars[..quote_idx]
        .iter()
        .rposition(|&c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '.')))
        .map_or(0, |i| i + 1);
    chars[token_start..quote_idx]
        .iter()
        .find(|&&c| c != '.')
        .is_some_and(|c| c.is_ascii_digit())
}

pub fn is_potential_definition(line: &Line, query: &str) -> bool {
    let line_text = &line.line;
    let line_lower = line_text.to_lowercase();
    let query_lower = query.to_lowercase();

    let is_constructor = if let Some(colon_pos) = query.rfind("::") {
        let class_part = &query[..colon_pos];
        let method_part = &query[colon_pos + 2..];
        let class_name = class_part.split("::").last().unwrap_or(class_part);
        if class_name != method_part {
            return false;
        }
        line_text.contains(&format!("::{}(", method_part))
            || (line_text
                .trim_start()
                .starts_with(&format!("{}(", method_part))
                && !line_text.contains("return"))
    } else {
        false
    };

    let contains_query =
        line_text.contains(query) || line_lower.contains(&query_lower) || is_constructor;

    if contains_query {
        let looks_like_definition = line_text.contains("{")
            || line_text.trim_end().ends_with(';')
            || line_text.contains("=")
            || line_text.contains("class ")
            || line_text.contains("struct ")
            || line_text.contains("interface ")
            || is_constructor
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

pub fn searchfox_url_repo(repo: &str) -> &str {
    // The firefox-* renames only exist on searchfox.org; a server behind
    // SEARCHFOX_BASE_URL serves trees under their configured names verbatim.
    if std::env::var("SEARCHFOX_BASE_URL").is_ok() {
        return repo;
    }
    match repo {
        "mozilla-central" => "firefox-main",
        "mozilla-beta" => "firefox-beta",
        "mozilla-release" => "firefox-release",
        "mozilla-esr128" => "firefox-esr128",
        "mozilla-esr140" => "firefox-esr140",
        _ => repo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_virtual_specifier_on_a_later_line() {
        let lines = [
            "  virtual void Continue(JSContext* aCx, JS::Handle<JS::Value> aKey,",
            "                        ErrorResult& aRv) = 0;",
        ];
        assert!(is_pure_virtual_declaration(&lines, 1));
        assert!(is_pure_virtual_declaration(
            &["  NS_IMETHOD GetFoo(bool* aFoo) const override=0;"],
            1
        ));
    }

    #[test]
    fn methods_with_bodies_are_not_pure_virtual() {
        assert!(!is_pure_virtual_declaration(
            &["  virtual nsContainerFrame* GetContentInsertionFrame() { return nullptr; }"],
            1
        ));
        let lines = [
            "void nsIFrame::Reflow(nsPresContext* aPresContext, ReflowOutput& aDesiredSize,",
            "                      const ReflowInput& aReflowInput, // = 0;",
            "                      nsReflowStatus& aStatus) {",
            "  aDesiredSize.ClearSize();",
            "}",
        ];
        assert!(!is_pure_virtual_declaration(&lines, 1));
    }

    #[test]
    fn default_argument_is_not_a_pure_specifier() {
        assert!(!is_pure_virtual_declaration(
            &["  virtual void Foo(uint32_t aFlags = 0);"],
            1
        ));
        assert!(!is_pure_virtual_declaration(
            &["  virtual void Foo() /* = 0 */;"],
            1
        ));
    }

    #[test]
    fn literals_in_default_arguments_are_not_syntax() {
        assert!(is_pure_virtual_declaration(
            &["  virtual void Foo(char aDelimiter = '(') = 0;"],
            1
        ));
        assert!(is_pure_virtual_declaration(
            &["  virtual void Foo(char aQuote = '\\'', char aClose = ')') = 0;"],
            1
        ));
        assert!(is_pure_virtual_declaration(
            &["  virtual void Foo(const char* aUrl = \"https://a.b/\\\"(\") = 0;"],
            1
        ));
        assert!(!is_pure_virtual_declaration(
            &["  virtual void Foo(const char* aText = \"= 0;\");"],
            1
        ));
        assert!(is_pure_virtual_declaration(
            &["  virtual void Foo(uint32_t aLimit = 1'000, uint32_t aMask = 0xFF'FF) = 0;"],
            1
        ));
        assert!(is_pure_virtual_declaration(
            &["  virtual void Foo(char8_t aOpen = u8'(', wchar_t aClose = L')') = 0;"],
            1
        ));
        assert!(is_pure_virtual_declaration(
            &["  virtual void Foo(double aScale = .5'5, double aHex = 0x1.F'Fp1) = 0;"],
            1
        ));
    }
}
