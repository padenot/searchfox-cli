use crate::client::SearchfoxClient;
use crate::types::{File, SearchfoxResponse};
use anyhow::Result;
use log::{debug, warn};
use reqwest::Url;

fn is_constructor_pattern(symbol: &str) -> bool {
    if let Some(colon_pos) = symbol.rfind("::") {
        let class_part = &symbol[..colon_pos];
        let method_part = &symbol[colon_pos + 2..];
        let class_name = class_part.split("::").last().unwrap_or(class_part);
        class_name == method_part
    } else {
        false
    }
}

fn extract_class_name_from_constructor(symbol: &str) -> String {
    if let Some(colon_pos) = symbol.rfind("::") {
        symbol[..colon_pos].to_string()
    } else {
        symbol.to_string()
    }
}

fn category_matches_symbol(category_name: &str, symbol: &str, case_sensitive: bool) -> bool {
    let Some(symbol_start) = category_name.find('(') else {
        return false;
    };
    let Some(symbol_end) = category_name.rfind(')') else {
        return false;
    };

    let category_symbol = &category_name[symbol_start + 1..symbol_end];
    let (category_symbol, symbol) = if case_sensitive {
        (category_symbol.to_string(), symbol.to_string())
    } else {
        (category_symbol.to_lowercase(), symbol.to_lowercase())
    };

    category_symbol == symbol || category_symbol.ends_with(&format!("::{symbol}"))
}

fn category_matches_constructor(category_name: &str, symbol: &str, case_sensitive: bool) -> bool {
    let Some(symbol_start) = category_name.find('(') else {
        return false;
    };
    let Some(symbol_end) = category_name.rfind(')') else {
        return false;
    };

    let category_symbol = &category_name[symbol_start + 1..symbol_end];
    let (category_symbol, symbol) = if case_sensitive {
        (category_symbol.to_string(), symbol.to_string())
    } else {
        (category_symbol.to_lowercase(), symbol.to_lowercase())
    };

    category_symbol == symbol || category_symbol.starts_with(&format!("{symbol}<"))
}

/// Filters search-result categories to matching source locations.
///
/// Categories must contain `search_type` and identify `symbol`; locations that
/// do not pass `options`' language filter are omitted. Returns `None` when no
/// matching locations remain.
fn filter_category_locations(
    json: &SearchfoxResponse,
    search_type: &str,
    symbol: &str,
    case_sensitive: bool,
    options: &SearchOptions,
) -> Option<Vec<(String, usize)>> {
    let mut locations = Vec::new();

    for (key, value) in json {
        if key.starts_with('*') {
            continue;
        }

        let Some(categories) = value.as_object() else {
            continue;
        };

        for (category_name, category_value) in categories {
            if !category_name.contains(search_type)
                || !category_matches_symbol(category_name, symbol, case_sensitive)
            {
                continue;
            }

            let Some(files_array) = category_value.as_array() else {
                continue;
            };

            for file in files_array {
                let Ok(file) = serde_json::from_value::<File>(file.clone()) else {
                    continue;
                };

                if !options.matches_language_filter(&file.path) {
                    continue;
                }

                for line in file.lines {
                    locations.push((file.path.clone(), line.lno));
                }
            }
        }
    }

    (!locations.is_empty()).then_some(locations)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Cpp,
    C,
    Js,
    WebIdl,
    Java,
    Kotlin,
    Rust,
    Python,
    Html,
    Css,
}

impl Lang {
    pub fn matches(&self, path: &str) -> bool {
        let p = path.to_lowercase();
        match self {
            Lang::Cpp => {
                p.ends_with(".cc")
                    || p.ends_with(".cpp")
                    || p.ends_with(".h")
                    || p.ends_with(".hh")
                    || p.ends_with(".hpp")
            }
            Lang::C => p.ends_with(".c") || p.ends_with(".h"),
            Lang::Js => {
                p.ends_with(".js")
                    || p.ends_with(".mjs")
                    || p.ends_with(".ts")
                    || p.ends_with(".cjs")
                    || p.ends_with(".jsx")
                    || p.ends_with(".tsx")
            }
            Lang::WebIdl => p.ends_with(".webidl"),
            Lang::Java | Lang::Kotlin => p.ends_with(".java") || p.ends_with(".kt"),
            Lang::Rust => p.ends_with(".rs"),
            Lang::Python => p.ends_with(".py"),
            Lang::Html => p.ends_with(".html") || p.ends_with(".xhtml") || p.ends_with(".htm"),
            Lang::Css => p.ends_with(".css"),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        // "c" is an alias for Cpp (same extensions in Mozilla's codebase).
        // "kotlin"/"kt" are aliases for Java (same filter: .java and .kt files).
        match s.to_lowercase().as_str() {
            "cpp" | "c++" | "c" => Some(Lang::Cpp),
            "js" | "javascript" | "typescript" | "ts" => Some(Lang::Js),
            "webidl" => Some(Lang::WebIdl),
            "java" | "kotlin" | "kt" => Some(Lang::Java),
            "rust" | "rs" => Some(Lang::Rust),
            "python" | "py" => Some(Lang::Python),
            "html" => Some(Lang::Html),
            "css" => Some(Lang::Css),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryFilter {
    All,
    ExcludeTests,
    ExcludeGenerated,
    ExcludeTestsAndGenerated,
    OnlyTests,
    OnlyGenerated,
    OnlyNormal,
}

impl CategoryFilter {
    pub fn should_include(&self, category: &str) -> bool {
        match self {
            CategoryFilter::All => true,
            CategoryFilter::ExcludeTests => category != "test",
            CategoryFilter::ExcludeGenerated => category != "generated",
            CategoryFilter::ExcludeTestsAndGenerated => {
                category != "test" && category != "generated"
            }
            CategoryFilter::OnlyTests => category == "test",
            CategoryFilter::OnlyGenerated => category == "generated",
            CategoryFilter::OnlyNormal => category == "normal",
        }
    }
}

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
    pub lang: Vec<Lang>,
    pub category_filter: CategoryFilter,
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
            lang: Vec::new(),
            category_filter: CategoryFilter::All,
        }
    }
}

impl SearchOptions {
    pub fn matches_language_filter(&self, path: &str) -> bool {
        if self.lang.is_empty() {
            return true;
        }
        self.lang.iter().any(|lang| lang.matches(path))
    }

    pub fn build_query(&self) -> String {
        if let Some(symbol) = &self.symbol {
            format!("symbol:{symbol}")
        } else if let Some(id) = &self.id {
            format!("id:{id}")
        } else if let Some(q) = &self.query {
            let has_prefix = q.contains("path:")
                || q.contains("pathre:")
                || q.contains("symbol:")
                || q.contains("id:")
                || q.contains("text:")
                || q.contains("re:");
            if let Some(context) = self.context {
                if has_prefix {
                    format!("context:{context} {q}")
                } else {
                    format!("context:{context} text:{q}")
                }
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
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

impl SearchfoxClient {
    pub async fn search(&self, options: &SearchOptions) -> Result<Vec<SearchResult>> {
        let query = options.build_query();

        let mut url = Url::parse(&format!("{}/{}/search", self.base_url, self.repo))?;
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

            if !options.category_filter.should_include(key) {
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
                            context_before: vec![],
                            context_after: vec![],
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
                                context_before: line.context_before.unwrap_or_default(),
                                context_after: line.context_after.unwrap_or_default(),
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
                                    context_before: vec![],
                                    context_after: vec![],
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
                                        context_before: line.context_before.unwrap_or_default(),
                                        context_after: line.context_after.unwrap_or_default(),
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
        let is_ctor = is_constructor_pattern(symbol);
        let search_symbol = if is_ctor {
            extract_class_name_from_constructor(symbol)
        } else {
            symbol.to_string()
        };
        // Exact identifier queries omit templated constructor categories such
        // as `nsTArray::nsTArray<E>`, so constructors use the full query.
        let query = if is_ctor {
            symbol.to_string()
        } else {
            format!("id:{search_symbol}")
        };
        let mut url = Url::parse(&format!("{}/{}/search", self.base_url, self.repo))?;
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

        let symbol_name = symbol.strip_prefix("id:").unwrap_or(symbol);
        let is_method_search = symbol_name.contains("::") && !is_ctor;

        // Try case-sensitive matches across all result buckets before falling back.
        if is_method_search {
            for case_sensitive in [true, false] {
                let find = |search_type| {
                    filter_category_locations(
                        &json,
                        search_type,
                        symbol_name,
                        case_sensitive,
                        options,
                    )
                };

                // When both exist, use overrides only if every definition is
                // detected as a pure-virtual declaration.
                match (find("Definitions"), find("Overridden By")) {
                    (Some(definitions), Some(overrides)) => {
                        let implementations =
                            self.without_pure_virtual_declarations(definitions).await;
                        return Ok(if implementations.is_empty() {
                            overrides
                        } else {
                            implementations
                        });
                    }
                    (Some(locations), None) | (None, Some(locations)) => return Ok(locations),
                    (None, None) => {}
                }

                if let Some(locations) = find("Declarations") {
                    return Ok(locations);
                }
            }
        }

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
                if !is_method_search && !is_ctor {
                    for (category_name, category_value) in categories {
                        let is_class_def_category = category_name.starts_with("Definitions (")
                            && (category_name.ends_with(&format!("::{symbol_name})"))
                                || category_name.ends_with(&format!("({symbol_name})")));
                        let is_not_constructor =
                            !category_name.contains(&format!("::{symbol_name}::{symbol_name})"));

                        if !is_class_def_category || !is_not_constructor {
                            continue;
                        }

                        debug!("Found class definition category: {}", category_name);
                        let Some(files_array) = category_value.as_array() else {
                            continue;
                        };

                        for file in files_array {
                            let Ok(file) = serde_json::from_value::<File>(file.clone()) else {
                                continue;
                            };

                            if !options.matches_language_filter(&file.path) {
                                continue;
                            }

                            let mut class_lines = Vec::new();
                            for line in file.lines {
                                if line.line.contains("class ") || line.line.contains("struct ") {
                                    debug!(
                                        "Found class/struct definition: {}:{} - {}",
                                        file.path,
                                        line.lno,
                                        line.line.trim()
                                    );
                                    class_lines.push((
                                        file.path.clone(),
                                        line.lno,
                                        line.line.clone(),
                                    ));
                                }
                            }

                            if class_lines.is_empty() {
                                continue;
                            }

                            for (path, lno, line_text) in &class_lines {
                                if !line_text.contains("{}") {
                                    return Ok(vec![(path.clone(), *lno)]);
                                }
                            }
                            let (path, lno, _) = &class_lines[0];
                            return Ok(vec![(path.clone(), *lno)]);
                        }
                    }
                }

                if is_ctor {
                    let mut all_ctor_lines = Vec::new();
                    for (category_name, category_value) in categories {
                        if category_name.contains("Definitions")
                            && category_matches_constructor(category_name, symbol, false)
                        {
                            debug!("Found constructor category: {}", category_name);
                            if let Some(files_array) = category_value.as_array() {
                                for file in files_array {
                                    match serde_json::from_value::<File>(file.clone()) {
                                        Ok(file) => {
                                            if !options.matches_language_filter(&file.path) {
                                                continue;
                                            }

                                            for line in file.lines {
                                                debug!(
                                                    "Found constructor definition: {}:{} - {}",
                                                    file.path,
                                                    line.lno,
                                                    line.line.trim()
                                                );
                                                all_ctor_lines.push((file.path.clone(), line.lno));
                                            }
                                        }
                                        Err(_) => continue,
                                    }
                                }
                            }
                        }
                    }

                    if !all_ctor_lines.is_empty() {
                        return Ok(all_ctor_lines);
                    }
                }

                let search_order = if is_method_search || is_ctor {
                    vec!["Definitions", "Declarations"]
                } else {
                    vec!["Declarations", "Definitions"]
                };

                for search_type in search_order {
                    for (category_name, category_value) in categories {
                        if category_name.contains(search_type)
                            && category_matches_symbol(category_name, symbol_name, false)
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

    /// Drops detected pure-virtual declarations; keeps locations on fetch errors.
    async fn without_pure_virtual_declarations(
        &self,
        locations: Vec<(String, usize)>,
    ) -> Vec<(String, usize)> {
        let mut implementations = Vec::new();
        for (path, lno) in locations {
            let is_pure_virtual = match self.get_file(&path).await {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    crate::utils::is_pure_virtual_declaration(&lines, lno)
                }
                Err(e) => {
                    debug!("Could not fetch {path} to check for a body: {e}");
                    false
                }
            };

            if is_pure_virtual {
                debug!("Skipping pure-virtual declaration: {path}:{lno}");
            } else {
                implementations.push((path, lno));
            }
        }
        implementations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Serves `lines` as the source of `file_path`, starting at `first_line`.
    async fn mount_source(server: &MockServer, file_path: &str, first_line: usize, lines: &[&str]) {
        let mut html = String::from("<html><body>");
        let padding = std::iter::repeat_n("", first_line - 1);
        for line in padding.chain(lines.iter().copied()) {
            let line = line
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            html.push_str(&format!("<code class=\"source-line\">{line}\n</code>"));
        }
        html.push_str("</body></html>");

        Mock::given(method("GET"))
            .and(path(format!("/firefox-main/source/{file_path}")))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(server)
            .await;
    }

    #[test]
    fn category_matches_exact_symbol_with_namespace() {
        assert!(category_matches_symbol(
            "Definitions (mozilla::dom::IDBCursor::Continue)",
            "IDBCursor::Continue",
            true,
        ));
        assert!(!category_matches_symbol(
            "Definitions (IDBCursor::continue)",
            "IDBCursor::Continue",
            true,
        ));
        assert!(category_matches_symbol(
            "Definitions (IDBCursor::continue)",
            "IDBCursor::Continue",
            false,
        ));
        assert!(!category_matches_symbol(
            "Definitions (IDBCursor::ContinuePrimaryKey)",
            "IDBCursor::Continue",
            false,
        ));
    }

    #[test]
    fn category_matches_templated_constructor() {
        assert!(category_matches_constructor(
            "Definitions (nsTArray::nsTArray<E>)",
            "nsTArray::nsTArray",
            false,
        ));
        assert!(!category_matches_constructor(
            "Definitions (nsTArray::nsTArrayExtra<E>)",
            "nsTArray::nsTArray",
            false,
        ));
    }

    #[tokio::test]
    async fn templated_constructor_search_uses_full_symbol_query() {
        let server = MockServer::start().await;
        let response = json!({
            "normal": {
                "Definitions (nsTArray::nsTArray<E>)": [{
                    "path": "xpcom/ds/nsTArray.h",
                    "lines": [{
                        "lno": 2636,
                        "line": "explicit nsTArray(size_type aCapacity) {}"
                    }]
                }],
                "Definitions (nsTArray::nsTArray<T>)": [{
                    "path": "xpcom/ds/nsTArray.h",
                    "lines": [{
                        "lno": 2635,
                        "line": "constexpr nsTArray() = default;"
                    }]
                }]
            }
        });

        Mock::given(method("GET"))
            .and(path("/mozilla-central/search"))
            .and(query_param("q", "nsTArray::nsTArray"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let mut locations = client
            .find_symbol_locations("nsTArray::nsTArray", None, &SearchOptions::default())
            .await
            .unwrap();
        locations.sort();

        assert_eq!(
            locations,
            vec![
                ("xpcom/ds/nsTArray.h".to_string(), 2635),
                ("xpcom/ds/nsTArray.h".to_string(), 2636),
            ]
        );
    }

    #[tokio::test]
    async fn implementation_category_precedes_declaration_and_case_insensitive_match() {
        let server = MockServer::start().await;
        let response = json!({
            "normal": {
                "Definitions (IDBCursor::continue)": [{
                    "path": "devtools/client/shared/sourceeditor/codemirror/codemirror.bundle.js",
                    "lines": [{
                        "lno": 1,
                        "line": "var CodeMirror = function() { continue: true; }"
                    }]
                }],
                "Definitions (mozilla::dom::IDBCursor::Continue)": [{
                    "path": "dom/indexedDB/IDBCursor.h",
                    "lines": [{
                        "lno": 127,
                        "line": "virtual void Continue(JSContext* aCx, JS::Handle<JS::Value> aKey,"
                    }]
                }],
                "Overridden By (mozilla::dom::IDBCursor::Continue)": [{
                    "path": "dom/indexedDB/IDBCursor.cpp",
                    "lines": [{
                        "lno": 336,
                        "line": "void IDBTypedCursor<CursorType>::Continue(JSContext* const aCx,",
                        "upsearch": "symbol:_ZN7mozilla3dom14IDBTypedCursor8ContinueEP9JSContext"
                    }]
                }, {
                    "path": "dom/indexedDB/IDBAnotherCursor.cpp",
                    "lines": [{
                        "lno": 400,
                        "line": "void IDBAnotherCursor::Continue(JSContext* const aCx,",
                        "upsearch": "symbol:_ZN7mozilla3dom16IDBAnotherCursor8ContinueEP9JSContext"
                    }]
                }]
            },
            "thirdparty": {
                "Definitions (IDBCursor::continue)": [{
                    "path": "devtools/client/shared/sourceeditor/codemirror/mode/javascript/javascript.js",
                    "lines": [{
                        "lno": 31,
                        "line": "\"continue\": kw(\"continue\")"
                    }]
                }]
            }
        });

        Mock::given(method("GET"))
            .and(path("/mozilla-central/search"))
            .and(query_param("q", "id:IDBCursor::Continue"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&server)
            .await;
        mount_source(
            &server,
            "dom/indexedDB/IDBCursor.h",
            127,
            &[
                "  virtual void Continue(JSContext* aCx, JS::Handle<JS::Value> aKey,",
                "                        ErrorResult& aRv) = 0;",
            ],
        )
        .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let locations = client
            .find_symbol_locations("IDBCursor::Continue", None, &SearchOptions::default())
            .await
            .unwrap();

        assert_eq!(
            locations,
            vec![
                ("dom/indexedDB/IDBCursor.cpp".to_string(), 336),
                ("dom/indexedDB/IDBAnotherCursor.cpp".to_string(), 400),
            ]
        );
    }

    #[tokio::test]
    async fn virtual_method_with_body_precedes_overrides() {
        let server = MockServer::start().await;
        let response = json!({
            "normal": {
                "Definitions (nsIFrame::Reflow)": [{
                    "path": "layout/generic/nsIFrame.cpp",
                    "lines": [{
                        "lno": 7696,
                        "line": "void nsIFrame::Reflow(nsPresContext* aPresContext, ReflowOutput& aDesiredSize,"
                    }]
                }],
                "Declarations (nsIFrame::Reflow)": [{
                    "path": "layout/generic/nsIFrame.h",
                    "lines": [{
                        "lno": 3224,
                        "line": "virtual void Reflow(nsPresContext* aPresContext, ReflowOutput& aReflowOutput,"
                    }]
                }],
                "Overridden By (nsIFrame::Reflow)": [{
                    "path": "layout/forms/nsCheckboxRadioFrame.cpp",
                    "lines": [{
                        "lno": 102,
                        "line": "void nsCheckboxRadioFrame::Reflow(nsPresContext* aPresContext,"
                    }]
                }]
            }
        });

        Mock::given(method("GET"))
            .and(path("/mozilla-central/search"))
            .and(query_param("q", "id:nsIFrame::Reflow"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&server)
            .await;
        mount_source(
            &server,
            "layout/generic/nsIFrame.cpp",
            7696,
            &[
                "void nsIFrame::Reflow(nsPresContext* aPresContext, ReflowOutput& aDesiredSize,",
                "                      const ReflowInput& aReflowInput,",
                "                      nsReflowStatus& aStatus) {",
                "  aDesiredSize.ClearSize();",
                "}",
            ],
        )
        .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let locations = client
            .find_symbol_locations("nsIFrame::Reflow", None, &SearchOptions::default())
            .await
            .unwrap();

        assert_eq!(
            locations,
            vec![("layout/generic/nsIFrame.cpp".to_string(), 7696)]
        );
    }

    #[tokio::test]
    async fn pure_virtual_overload_is_dropped_from_definitions_with_bodies() {
        let server = MockServer::start().await;
        let response = json!({
            "normal": {
                "Definitions (mozilla::dom::EventTarget::DispatchEvent)": [{
                    "path": "dom/events/EventTarget.cpp",
                    "lines": [{
                        "lno": 210,
                        "line": "void EventTarget::DispatchEvent(Event& aEvent) {"
                    }, {
                        "lno": 214,
                        "line": "void EventTarget::DispatchEvent(Event& aEvent, ErrorResult& aRv) {"
                    }]
                }, {
                    "path": "dom/events/EventTarget.h",
                    "lines": [{
                        "lno": 188,
                        "line": "MOZ_CAN_RUN_SCRIPT_BOUNDARY virtual bool DispatchEvent(Event& aEvent,"
                    }]
                }],
                "Overridden By (mozilla::dom::EventTarget::DispatchEvent)": [{
                    "path": "dom/base/nsINode.cpp",
                    "lines": [{
                        "lno": 1416,
                        "line": "bool nsINode::DispatchEvent(Event& aEvent, CallerType aCallerType,"
                    }]
                }]
            }
        });

        Mock::given(method("GET"))
            .and(path("/mozilla-central/search"))
            .and(query_param(
                "q",
                "id:mozilla::dom::EventTarget::DispatchEvent",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&server)
            .await;
        mount_source(
            &server,
            "dom/events/EventTarget.cpp",
            210,
            &[
                "void EventTarget::DispatchEvent(Event& aEvent) {",
                "  (void)DispatchEvent(aEvent, CallerType::NonSystem, IgnoreErrors());",
                "}",
                "",
                "void EventTarget::DispatchEvent(Event& aEvent, ErrorResult& aRv) {",
                "  (void)DispatchEvent(aEvent, CallerType::NonSystem, aRv);",
                "}",
            ],
        )
        .await;
        mount_source(
            &server,
            "dom/events/EventTarget.h",
            188,
            &[
                "  MOZ_CAN_RUN_SCRIPT_BOUNDARY virtual bool DispatchEvent(Event& aEvent,",
                "                                                         CallerType aCallerType,",
                "                                                         ErrorResult& aRv) = 0;",
            ],
        )
        .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let locations = client
            .find_symbol_locations(
                "mozilla::dom::EventTarget::DispatchEvent",
                None,
                &SearchOptions::default(),
            )
            .await
            .unwrap();

        assert_eq!(
            locations,
            vec![
                ("dom/events/EventTarget.cpp".to_string(), 210),
                ("dom/events/EventTarget.cpp".to_string(), 214),
            ]
        );
    }
}
