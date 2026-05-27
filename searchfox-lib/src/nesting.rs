use crate::client::SearchfoxClient;
use crate::utils::searchfox_url_repo;
use anyhow::Result;
use scraper::{ElementRef, Html, Selector};

/// One level of nesting context: the symbol name and the source text of the opening line.
#[derive(Debug, Clone)]
pub struct NestingContext {
    pub sym: String,
    pub pretty_line: String,
}

impl SearchfoxClient {
    /// Return the nesting context (function/class/namespace chain) that contains `line` in `path`.
    ///
    /// The returned Vec is ordered from innermost to outermost. Returns an empty Vec when the
    /// line is at file scope (not inside any nesting block).
    pub async fn get_function_at_line(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Vec<NestingContext>> {
        let url = format!(
            "{}/{}/source/{}",
            self.base_url,
            searchfox_url_repo(&self.repo),
            path
        );
        let html = self.get_html(&url).await?;
        Ok(parse_nesting_at_line(&html, line))
    }
}

fn parse_nesting_at_line(html: &str, target_line: usize) -> Vec<NestingContext> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse(&format!("div[id=\"line-{}\"]", target_line)).unwrap();
    let sticky_selector = Selector::parse("code.source-line").unwrap();

    let row = match document.select(&row_selector).next() {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut contexts = Vec::new();
    let mut node = row.parent();
    while let Some(parent) = node {
        if let Some(elem) = ElementRef::wrap(parent) {
            let val = elem.value();
            if val.has_class("nesting-container", scraper::CaseSensitivity::CaseSensitive) {
                if let Some(sym) = val.attr("data-nesting-sym") {
                    let pretty_line = elem
                        .select(&sticky_selector)
                        .next()
                        .map(|code| code.text().collect::<String>().trim().to_string())
                        .unwrap_or_default();
                    contexts.push(NestingContext {
                        sym: sym.to_string(),
                        pretty_line,
                    });
                }
            }
        }
        node = parent.parent();
    }

    contexts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_html(inner_line: &str) -> String {
        format!(
            r#"<html><body>
<div class="nesting-container nesting-depth-1" data-nesting-sym="OuterClass">
  <div role="row" id="line-1" class="source-line-with-number nesting-sticky-line">
    <code role="cell" class="source-line">class OuterClass {{</code>
  </div>
  <div class="nesting-container nesting-depth-2" data-nesting-sym="OuterClass::innerMethod">
    <div role="row" id="line-2" class="source-line-with-number nesting-sticky-line">
      <code role="cell" class="source-line">  void innerMethod() {{</code>
    </div>
    <div role="row" id="line-3" class="source-line-with-number">
      <code role="cell" class="source-line">{}</code>
    </div>
  </div>
</div>
<div role="row" id="line-4" class="source-line-with-number">
  <code role="cell" class="source-line">// file scope</code>
</div>
</body></html>"#,
            inner_line
        )
    }

    #[test]
    fn line_inside_inner_nesting() {
        let html = make_html("    return 42;");
        let result = parse_nesting_at_line(&html, 3);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sym, "OuterClass::innerMethod");
        assert_eq!(result[1].sym, "OuterClass");
    }

    #[test]
    fn line_at_file_scope() {
        let html = make_html("    return 42;");
        let result = parse_nesting_at_line(&html, 4);
        assert!(result.is_empty());
    }

    #[test]
    fn line_not_found() {
        let html = make_html("    return 42;");
        let result = parse_nesting_at_line(&html, 99);
        assert!(result.is_empty());
    }

    #[test]
    fn sticky_line_text_captured() {
        let html = make_html("    return 42;");
        let result = parse_nesting_at_line(&html, 3);
        assert!(!result[0].pretty_line.is_empty());
        assert!(result[0].pretty_line.contains("innerMethod"));
    }
}
