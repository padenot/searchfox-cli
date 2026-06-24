use crate::client::SearchfoxClient;
use crate::search::{SearchOptions, SearchResult};
use anyhow::Result;
use url::Url;

const SPEC_REF_PATH_CATEGORIES: &[(&str, &str)] = &[
    ("testing/web-platform", "Web-Platform Test"),
    ("js/src/tests/test262", "Test262"),
    ("js/src/jit-test/tests/wasm", "WebAssembly Test"),
];

pub fn categorize_spec_ref(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    for (prefix, category) in SPEC_REF_PATH_CATEGORIES {
        if lower.starts_with(prefix) {
            return category;
        }
    }
    if lower.contains("test") {
        return "Test";
    }
    "Code"
}

pub fn spec_ref_category_names() -> &'static [&'static str] {
    &[
        "Code",
        "Test",
        "Test262",
        "WebAssembly Test",
        "Web-Platform Test",
    ]
}

pub fn spec_refs_query(spec_url: &str) -> Result<String> {
    let parsed = Url::parse(spec_url).map_err(|_| anyhow::anyhow!("Invalid URL: {spec_url}"))?;
    let domain = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("No host in URL: {spec_url}"))?
        .to_string();
    let anchor = parsed
        .fragment()
        .ok_or_else(|| anyhow::anyhow!("URL must contain a #fragment: {spec_url}"))?
        .to_string();
    Ok(format!(
        "re:{}[^\\s]*#{}\\b",
        regex::escape(&domain),
        regex::escape(&anchor)
    ))
}

impl SearchfoxClient {
    pub async fn search_spec_refs(
        &self,
        spec_url: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let query = spec_refs_query(spec_url)?;
        let options = SearchOptions {
            query: Some(query),
            limit,
            ..Default::default()
        };
        self.search(&options).await
    }
}
