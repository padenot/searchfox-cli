use clap::Parser;
use log::{debug, error, warn};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn get_user_agent() -> String {
    let magic_word =
        std::env::var("SEARCHFOX_MAGIC_WORD").unwrap_or_else(|_| "sésame ouvre toi".to_string());
    format!("searchfox-cli/{} ({})", VERSION, magic_word)
}

fn create_tls13_client() -> anyhow::Result<Client> {
    Client::builder()
        .user_agent(get_user_agent())
        .use_rustls_tls()
        .min_tls_version(reqwest::tls::Version::TLS_1_2)
        .max_tls_version(reqwest::tls::Version::TLS_1_3)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build TLS client with rustls: {}", e))
}

#[derive(Parser, Debug)]
#[command(
    name = "searchfox-cli",
    about = "Searchfox CLI for Mozilla code search",
    long_about = "A command-line interface for searching Mozilla codebases using searchfox.org.\n\nExamples:\n  searchfox-cli -q AudioStream\n  searchfox-cli -q AudioStream -C -l 10\n  searchfox-cli -q '^Audio.*' -r\n  searchfox-cli -q AudioStream -p ^dom/media\n  searchfox-cli -p PContent.ipdl  # Search for files by path only\n  searchfox-cli --get-file dom/media/AudioStream.h\n  searchfox-cli --symbol AudioContext\n  searchfox-cli --symbol 'AudioContext::CreateGain'\n  searchfox-cli --id main\n  searchfox-cli -q 'path:dom/media AudioStream'\n  searchfox-cli -q 'symbol:AudioContext' --context 3\n  searchfox-cli --define 'AudioContext::CreateGain'\n  searchfox-cli --calls-from 'mozilla::dom::AudioContext::CreateGain' --depth 2\n  searchfox-cli --calls-to 'mozilla::dom::AudioContext::CreateGain' --depth 3\n  searchfox-cli --calls-between 'AudioContext,AudioNode' --depth 2"
)]
struct Args {
    #[arg(short, long, help = "Search query string")]
    query: Option<String>,

    #[arg(
        short = 'R',
        long,
        default_value = "mozilla-central",
        help = "Repository to search in",
        long_help = "Repository to search in. Available repositories:\n  mozilla-central (default) - Main Firefox development\n  autoland - Integration repository\n  mozilla-beta - Beta release branch\n  mozilla-release - Release branch\n  mozilla-esr115 - ESR 115 branch\n  mozilla-esr128 - ESR 128 branch\n  mozilla-esr140 - ESR 140 branch\n  comm-central - Thunderbird development"
    )]
    repo: String,

    #[arg(
        short,
        long,
        help = "Filter results by path prefix (e.g., ^dom/media) or search for files by path",
        long_help = "Filter search results by file path prefix or search for files by path pattern.\nUse regex patterns to match specific directories or files.\nCan be used alone to search for files without a query.\nExamples:\n  -p ^dom/media (with query) - filters results to files starting with dom/media/\n  -p PContent.ipdl (alone) - finds all files matching PContent.ipdl"
    )]
    path: Option<String>,

    #[arg(
        short = 'C',
        long,
        default_value_t = false,
        help = "Enable case-sensitive search"
    )]
    case: bool,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Enable regular expression search",
        long_help = "Enable regular expression search mode.\nAllows using regex patterns in the query string.\nExample: '^Audio.*' matches identifiers starting with 'Audio'"
    )]
    regexp: bool,

    #[arg(
        short,
        long,
        default_value_t = 50,
        help = "Maximum number of results to display"
    )]
    limit: usize,

    #[arg(
        long,
        help = "Fetch and display the contents of a specific file",
        long_help = "Fetch and display the contents of a specific file from the repository.\nProvide the file path relative to the repository root.\nExample: --get-file dom/media/AudioStream.h"
    )]
    get_file: Option<String>,

    #[arg(
        long,
        help = "Number of context lines to show around matches",
        long_help = "Show N lines of context above and below each match.\nOnly works with text: or re: queries.\nExample: --context 3"
    )]
    context: Option<usize>,

    #[arg(
        long,
        help = "Find symbol definitions",
        long_help = "Search for symbol definitions using searchfox's symbol indexing.\nUses symbol: query syntax internally.\nExample: --symbol AudioContext"
    )]
    symbol: Option<String>,

    #[arg(
        long,
        help = "Find exact identifier matches",
        long_help = "Search for exact identifier matches (not prefix-based).\nUses id: query syntax internally.\nExample: --id main"
    )]
    id: Option<String>,

    #[arg(
        long,
        help = "Find and display the definition of a symbol",
        long_help = "Find the definition of a symbol using searchfox's structured data.\nSearches for symbol definitions and class/struct declarations.\nDisplays the complete method/function body or class declaration.\nExample: --define 'AudioContext::CreateGain' or --define 'AudioContext'"
    )]
    define: Option<String>,

    #[arg(
        long,
        help = "Enable request logging with timing and size information",
        long_help = "Log all HTTP requests made to searchfox with detailed timing information:\n- Request start/end timestamps\n- Response size and duration\n- Network latency measurement via ping\nUseful for performance analysis and infrastructure planning"
    )]
    log_requests: bool,

    #[arg(
        long = "cpp",
        help = "Filter results to C++ files only",
        long_help = "Filter results to C++ files only (.cc, .cpp, .h, .hh, .hpp)"
    )]
    cpp: bool,

    #[arg(
        long = "c",
        help = "Filter results to C files only",
        long_help = "Filter results to C files only (.c, .h)"
    )]
    c_lang: bool,

    #[arg(
        long = "webidl",
        help = "Filter results to WebIDL files only",
        long_help = "Filter results to WebIDL files only (.webidl)"
    )]
    webidl: bool,

    #[arg(
        long = "js",
        help = "Filter results to JavaScript files only",
        long_help = "Filter results to JavaScript files only (.js, .mjs, .ts, .cjs, .jsx, .tsx)"
    )]
    js: bool,

    #[arg(
        long = "calls-from",
        help = "Find functions called by the specified symbol",
        long_help = "Search for functions called by the specified symbol using call graph analysis.\nExample: --calls-from 'mozilla::dom::AudioContext::CreateGain'"
    )]
    calls_from: Option<String>,

    #[arg(
        long = "calls-to",
        help = "Find functions that call the specified symbol",
        long_help = "Search for functions that call the specified symbol using call graph analysis.\nExample: --calls-to 'mozilla::dom::AudioContext::CreateGain'"
    )]
    calls_to: Option<String>,

    #[arg(
        long = "calls-between",
        help = "Find function calls between two symbols or classes",
        long_help = "Find function calls between two symbols or classes.\nExample: --calls-between 'AudioContext,AudioNode'"
    )]
    calls_between: Option<String>,

    #[arg(
        long = "depth",
        default_value_t = 2,
        help = "Set traversal depth for call graph searches",
        long_help = "Set the depth of traversal for call graph searches. Higher values show more indirect calls.\nExample: --depth 3"
    )]
    depth: u32,
}

#[derive(Debug, Deserialize)]
struct Line {
    lno: usize,
    line: String,
    #[allow(dead_code)]
    bounds: Option<Vec<usize>>,
    #[allow(dead_code)]
    context: Option<String>,
    #[allow(dead_code)]
    contextsym: Option<String>,
    #[serde(rename = "peekRange")]
    #[allow(dead_code)]
    peek_range: Option<String>,
    upsearch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct File {
    path: String,
    lines: Vec<Line>,
}

// Allow dynamic keys in top-level object
type SearchfoxResponse = HashMap<String, serde_json::Value>;

/// Request logging information
#[derive(Debug)]
struct RequestLog {
    url: String,
    method: String,
    start_time: Instant,
    request_id: usize,
}

/// Response logging information
#[derive(Debug)]
#[allow(dead_code)]
struct ResponseLog {
    request_id: usize,
    status: u16,
    size_bytes: usize,
    duration: Duration,
}

static mut REQUEST_COUNTER: usize = 0;

/// Log the start of a request
fn log_request_start(method: &str, url: &str) -> RequestLog {
    unsafe {
        REQUEST_COUNTER += 1;
        let log = RequestLog {
            url: url.to_string(),
            method: method.to_string(),
            start_time: Instant::now(),
            request_id: REQUEST_COUNTER,
        };

        eprintln!(
            "[REQ-{}] {} {} - START",
            log.request_id, log.method, log.url
        );
        log
    }
}

/// Log the completion of a request
fn log_request_end(request_log: RequestLog, status: u16, size_bytes: usize) {
    let duration = request_log.start_time.elapsed();

    let response_log = ResponseLog {
        request_id: request_log.request_id,
        status,
        size_bytes,
        duration,
    };

    eprintln!(
        "[REQ-{}] {} {} - END ({}ms, {} bytes, HTTP {})",
        response_log.request_id,
        request_log.method,
        request_log.url,
        duration.as_millis(),
        size_bytes,
        status
    );
}

/// Measure baseline network latency using a simple HTTP HEAD request
async fn ping_searchfox(
    client: &Client,
    _repo: &str,
) -> Result<Duration, Box<dyn std::error::Error>> {
    eprintln!(
        "[PING] Testing network latency to searchfox.org (ICMP ping disabled, using HTTP HEAD)..."
    );

    let ping_url = "https://searchfox.org/";
    let start = Instant::now();

    let response = client
        .head(ping_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let latency = start.elapsed();

    eprintln!(
        "[PING] HTTP HEAD latency: {}ms (HTTP {})",
        latency.as_millis(),
        response.status()
    );
    eprintln!(
        "[PING] Note: This includes minimal server processing time, not just network latency"
    );

    Ok(latency)
}

/// A simpler approach: Instead of trying to extract mangled symbols from HTML,
/// let's search for the symbol and then visit individual source pages to find
/// the actual definitions with their symbols
async fn find_symbol_in_search_results(
    client: &Client,
    repo: &str,
    query: &str,
    path_filter: Option<&str>,
    args: &Args,
) -> anyhow::Result<Vec<(String, usize)>> {
    // First get JSON search results to find files containing the symbol
    let mut url = Url::parse(&format!("https://searchfox.org/{repo}/search"))?;
    url.query_pairs_mut().append_pair("q", query);
    if let Some(path) = path_filter {
        url.query_pairs_mut().append_pair("path", path);
    }

    let request_log = if args.log_requests {
        Some(log_request_start("GET", url.as_ref()))
    } else {
        None
    };

    let response = client
        .get(url.clone())
        .header("Accept", "application/json")
        .send()
        .await?;

    if !response.status().is_success() {
        if let Some(req_log) = request_log {
            log_request_end(req_log, response.status().as_u16(), 0);
        }
        anyhow::bail!("Request failed: {}", response.status());
    }

    let response_text = response.text().await?;
    let response_size = response_text.len();

    if let Some(req_log) = request_log {
        log_request_end(req_log, 200, response_size);
    }

    let json: SearchfoxResponse = serde_json::from_str(&response_text)?;
    let mut file_locations = Vec::new();

    debug!("Analyzing search results...");

    // Extract file paths and line numbers from search results
    for (key, value) in &json {
        if key.starts_with('*') {
            continue; // skip metadata
        }
        if let Some(files_array) = value.as_array() {
            debug!("Found {} files in array for key {}", files_array.len(), key);
            for file in files_array {
                match serde_json::from_value::<File>(file.clone()) {
                    Ok(file) => {
                        // Skip files that don't match language filters
                        if !matches_language_filter(&file.path, args) {
                            continue;
                        }

                        debug!(
                            "Processing file: {} with {} lines",
                            file.path,
                            file.lines.len()
                        );
                        for line in file.lines {
                            // Look for definitions (lines that contain the query term and look like definitions)
                            let line_text = &line.line;
                            let line_lower = line_text.to_lowercase();
                            let query_lower = query.to_lowercase();

                            // Check if this line contains our query term
                            let contains_query =
                                line_text.contains(query) || line_lower.contains(&query_lower);

                            if contains_query {
                                // Look for definition patterns
                                let looks_like_definition = line_text.contains("{") ||           // function body start
                                    line_text.trim_end().ends_with(';') ||  // declaration
                                    line_text.contains("=") ||           // assignment/initialization
                                    line_text.contains("class ") ||      // class declaration
                                    line_text.contains("struct ") ||     // struct declaration
                                    line_text.contains("interface ") ||  // interface declaration
                                    (line_text.contains("::") && (       // C++ method definition
                                        line_text.contains("(") ||       // function call/definition
                                        line_text.contains("already_AddRefed") ||  // Mozilla return type
                                        line_text.contains("RefPtr") ||  // Mozilla smart pointer
                                        line_text.contains("nsCOMPtr")   // Mozilla COM pointer
                                    ));

                                if looks_like_definition {
                                    debug!(
                                        "Found potential definition: {}:{} - {}",
                                        file.path,
                                        line.lno,
                                        line_text.trim()
                                    );
                                    file_locations.push((file.path.clone(), line.lno));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse file JSON: {e}");
                        debug!(
                            "File JSON: {}",
                            serde_json::to_string_pretty(&file)
                                .unwrap_or_else(|_| "Failed to serialize".to_string())
                        );
                    }
                }
            }
        } else if let Some(categories) = value.as_object() {
            // Extract the symbol name from the query (remove "id:" prefix if present)
            let symbol_name = query.strip_prefix("id:").unwrap_or(query);

            // Determine if this is a method/constructor search (contains ::) or class search
            let is_method_search = symbol_name.contains("::");

            if !is_method_search {
                // For class searches, look for class/struct definitions first
                let class_def_key = format!("Definitions ({symbol_name})");
                if let Some(files_array) = categories.get(&class_def_key).and_then(|v| v.as_array())
                {
                    for file in files_array {
                        match serde_json::from_value::<File>(file.clone()) {
                            Ok(file) => {
                                // Skip files that don't match language filters
                                if !matches_language_filter(&file.path, args) {
                                    continue;
                                }

                                for line in file.lines {
                                    // Check if this is a class/struct definition
                                    if line.line.contains("class ") || line.line.contains("struct ")
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

            // Then look for other definitions and declarations
            // For method searches, prioritize "Definitions" over "Declarations"
            let search_order = if is_method_search {
                vec!["Definitions", "Declarations"]
            } else {
                vec!["Declarations", "Definitions"] // For classes, declarations might also be useful
            };

            for search_type in search_order {
                for (category_name, category_value) in categories {
                    // Skip the class definition key we already processed for non-method searches
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
                                        // Skip files that don't match language filters
                                        if !matches_language_filter(&file.path, args) {
                                            continue;
                                        }

                                        for line in file.lines {
                                            // Check if we have an upsearch symbol (this is the mangled symbol we need)
                                            if let Some(upsearch) = &line.upsearch {
                                                if upsearch.starts_with("symbol:_Z") {
                                                    // Use this mangled symbol directly for a more targeted search
                                                    return Ok(vec![(file.path.clone(), line.lno)]);
                                                }
                                            }
                                            file_locations.push((file.path.clone(), line.lno));
                                        }
                                    }
                                    Err(_) => {
                                        // Skip files that don't parse correctly
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }

                // If we found results with this search type, stop searching
                if !file_locations.is_empty() {
                    break;
                }
            }
        }
    }

    Ok(file_locations)
}

/// Extract mangled symbols from a specific source file page
#[allow(dead_code)]
async fn extract_symbols_from_source_page(
    client: &Client,
    repo: &str,
    file_path: &str,
    line_number: usize,
) -> anyhow::Result<Vec<String>> {
    let url = format!("https://searchfox.org/{repo}/source/{file_path}#{line_number}");

    debug!("Fetching source page: {url}");

    let response = client
        .get(&url)
        .header("Accept", "text/html")
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let html_content = response.text().await?;
    let document = Html::parse_document(&html_content);
    let mut mangled_symbols = Vec::new();

    // Look for elements that might contain symbol information around the target line
    let line_id = format!("#{line_number}");
    let line_id_alt = format!("#line-{line_number}");
    let selectors_to_try = [
        line_id.as_str(),
        line_id_alt.as_str(),
        ".target-line",
        "[data-symbols]",
        "[data-id]",
        "span[title]",
        ".syn_def",
        ".syn_type",
    ];

    for selector_str in &selectors_to_try {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                // Check all attributes for symbol data
                for (attr_name, attr_value) in element.value().attrs() {
                    if attr_name.contains("symbol")
                        || attr_name.contains("data")
                        || attr_name == "title"
                    {
                        debug!("Found attribute {attr_name}: {attr_value}");

                        // Extract mangled symbols from attribute values
                        for word in attr_value.split(|c: char| !c.is_alphanumeric() && c != '_') {
                            if word.starts_with("_Z") && word.len() > 10 {
                                debug!("Found mangled symbol: {word}");
                                mangled_symbols.push(word.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Also search the entire HTML for mangled symbols as a fallback
    for word in html_content.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.starts_with("_Z") && word.len() > 15 && word.len() < 200 {
            mangled_symbols.push(word.to_string());
        }
    }

    // Remove duplicates
    mangled_symbols.sort();
    mangled_symbols.dedup();

    debug!(
        "Extracted {} symbols from source page",
        mangled_symbols.len()
    );
    Ok(mangled_symbols)
}

/// Helper function to extract symbols from JSON data
#[allow(dead_code)]
fn extract_symbols_from_json(json: &serde_json::Value, symbols: &mut Vec<String>) {
    match json {
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_symbols_from_json(item, symbols);
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, value) in obj {
                if key == "sym" || key == "symbol" || key == "id" {
                    if let Some(sym_str) = value.as_str() {
                        if sym_str.starts_with("_Z") {
                            symbols.push(sym_str.to_string());
                        }
                    }
                }
                extract_symbols_from_json(value, symbols);
            }
        }
        serde_json::Value::String(s) => {
            if s.starts_with("_Z") {
                symbols.push(s.clone());
            }
        }
        _ => {}
    }
}

/// Find the definition of a symbol using its mangled form
#[allow(dead_code)]
async fn find_symbol_definition(
    client: &Client,
    repo: &str,
    mangled_symbol: &str,
) -> anyhow::Result<Option<(String, String, usize)>> {
    let query = format!("symbol:{mangled_symbol}");
    let mut url = Url::parse(&format!("https://searchfox.org/{repo}/search"))?;
    url.query_pairs_mut().append_pair("q", &query);

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let json: SearchfoxResponse = response.json().await?;

    // Look for definitions in the search results
    for (key, value) in &json {
        if key.starts_with('*') {
            continue; // skip metadata
        }

        if let Some(files_array) = value.as_array() {
            for file in files_array {
                let file: File = serde_json::from_value(file.clone())?;
                for line in file.lines {
                    // Look for definition patterns (function definitions, not just calls)
                    let line_text = &line.line;
                    if line_text.contains("::")
                        && (line_text.contains("{")
                            || line_text.trim_end().ends_with(';')
                            || line_text.contains("="))
                    {
                        return Ok(Some((file.path.clone(), line_text.clone(), line.lno)));
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Extract the complete method/function body by parsing braces
fn extract_complete_method(lines: &[&str], start_line: usize) -> (usize, Vec<String>) {
    let start_idx = start_line.saturating_sub(1); // Convert to 0-based index
    if start_idx >= lines.len() {
        return (
            start_line,
            vec![lines.get(start_idx).unwrap_or(&"").to_string()],
        );
    }

    let start_line_content = lines[start_idx];

    // Check if this looks like a function/method definition or class/struct definition
    // Look for patterns like:
    // - Class::method(...)
    // - return_type Class::method(...)
    // - Class::Class(...) (constructor)
    // - function_name(...)
    // - class ClassName
    // - struct StructName
    let looks_like_function = (start_line_content.contains("(")
        && (start_line_content.contains("{") || 
                              start_line_content.trim_end().ends_with(")") ||
                              start_line_content.trim_end().ends_with(";") ||
                              start_line_content.contains("::") ||  // C++ method/constructor
                              start_line_content.trim_start().starts_with("fn ") ||  // Rust function
                              start_line_content.contains("function "))) // JavaScript function
        || start_line_content.contains("class ")  // C++ class
        || start_line_content.contains("struct ") // C++ struct
        || start_line_content.contains("interface "); // interface

    if !looks_like_function {
        // Check if this could be a multi-line function signature
        // Look ahead a few lines to see if we find opening braces or initializer lists
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
            // Not a function, return just context
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

    // If the function declaration ends with ';', it's just a declaration
    // Exception: class/struct forward declarations like "class Foo;"
    if start_line_content.trim_end().ends_with(';')
        && !(start_line_content.contains("class ") || start_line_content.contains("struct "))
    {
        return (
            start_line,
            vec![format!(">>> {:4}: {}", start_line, start_line_content)],
        );
    }

    // Find the opening brace
    let mut found_opening_brace = start_line_content.contains('{');

    if !found_opening_brace {
        // Look for opening brace in subsequent lines
        // For constructors, we might have initializer lists that start with ':'
        for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
            if line.contains('{') {
                found_opening_brace = true;
                break;
            }
            // Stop searching if we hit another function or EOF
            // Allow more lines for complex constructors with initializer lists
            // Look for other function signatures as a stop condition
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
        // No opening brace found, treat as declaration
        return (
            start_line,
            vec![format!(">>> {:4}: {}", start_line, start_line_content)],
        );
    }

    // Parse braces to find the complete method body
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

        // Parse characters to track braces while ignoring those in strings/comments
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
                        j += 1; // Skip the second '/'
                    } else if next_ch == Some('*') {
                        in_multi_comment = true;
                        j += 1; // Skip the '*'
                    }
                }
                '*' if in_multi_comment && next_ch == Some('/') => {
                    in_multi_comment = false;
                    j += 1; // Skip the '/'
                }
                '{' if !in_string && !in_char && !in_single_comment && !in_multi_comment => {
                    brace_count += 1;
                }
                '}' if !in_string && !in_char && !in_single_comment && !in_multi_comment => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        // Found the end of the method/class
                        // For classes/structs, check if there's a semicolon after the closing brace
                        let is_class_or_struct = lines[start_idx].contains("class ")
                            || lines[start_idx].contains("struct ");
                        if is_class_or_struct {
                            // Look for semicolon on same line or next line
                            let remaining_on_line = &line[j + 1..];
                            if remaining_on_line.trim().starts_with(';') {
                                // Include the semicolon in the output
                                return (start_line, result_lines);
                            } else if i + 1 < lines.len() {
                                // Check next line for semicolon
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

        // Reset single-line comment flag at end of line
        in_single_comment = false;

        // Safety check: don't extract more than 200 lines
        if result_lines.len() > 200 {
            result_lines.push("   ...  : (method too long, truncated)".to_string());
            break;
        }
    }

    (start_line, result_lines)
}

/// Check if we're in a Mozilla repository by looking for the mach file
fn is_mozilla_repository() -> bool {
    std::path::Path::new("./mach").exists()
}

/// Check if a file path matches the language filters
fn matches_language_filter(path: &str, args: &Args) -> bool {
    // If no language filters are specified, include all files
    if !args.cpp && !args.c_lang && !args.webidl && !args.js {
        return true;
    }

    let path_lower = path.to_lowercase();

    // Check C++ files
    if args.cpp
        && (path_lower.ends_with(".cc")
            || path_lower.ends_with(".cpp")
            || path_lower.ends_with(".h")
            || path_lower.ends_with(".hh")
            || path_lower.ends_with(".hpp"))
    {
        return true;
    }

    // Check C files
    if args.c_lang && (path_lower.ends_with(".c") || path_lower.ends_with(".h")) {
        return true;
    }

    // Check WebIDL files
    if args.webidl && path_lower.ends_with(".webidl") {
        return true;
    }

    // Check JavaScript files
    if args.js
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

/// Try to read a file locally from the repository
fn read_local_file(file_path: &str) -> Option<String> {
    // Try to read the file from the current directory
    if let Ok(content) = std::fs::read_to_string(file_path) {
        return Some(content);
    }

    // If that fails, try with ./ prefix
    if let Ok(content) = std::fs::read_to_string(format!("./{file_path}")) {
        return Some(content);
    }

    None
}

/// Find a symbol definition in local file content, allowing for line number drift
fn find_symbol_in_local_content(
    content: &str,
    expected_line: usize,
    symbol: &str,
) -> Option<usize> {
    let lines: Vec<&str> = content.lines().collect();

    // First check the expected line
    if expected_line > 0 && expected_line <= lines.len() {
        let line_idx = expected_line - 1;
        if lines[line_idx].contains(symbol)
            || (symbol.contains("::")
                && lines[line_idx].contains(symbol.split("::").last().unwrap_or("")))
        {
            return Some(expected_line);
        }
    }

    // Search within a reasonable range around the expected line
    let search_range = 50; // Look 50 lines above and below
    let start = expected_line.saturating_sub(search_range);
    let end = std::cmp::min(expected_line + search_range, lines.len());

    for i in start..end {
        if i < lines.len() {
            let line = lines[i];
            // Check if this line contains the symbol or looks like a definition
            if (line.contains(symbol)
                || (symbol.contains("::")
                    && line.contains(symbol.split("::").last().unwrap_or(""))))
                && (line.contains("::") || line.contains("(") || line.contains("="))
            {
                return Some(i + 1); // Convert back to 1-based line number
            }
        }
    }

    None
}

/// Get the definition context (multiple lines around the definition)
async fn get_definition_context(
    client: &Client,
    repo: &str,
    file_path: &str,
    line_number: usize,
    context_lines: usize,
    symbol_name: Option<&str>,
    log_requests: bool,
) -> anyhow::Result<String> {
    // Try local file reading first if we're in a Mozilla repository
    if is_mozilla_repository() {
        if let Some(local_content) = read_local_file(file_path) {
            let lines: Vec<&str> = local_content.lines().collect();

            // Check if we can find the symbol at or near the expected line
            let actual_line = if line_number > 0 && line_number <= lines.len() {
                // Quick check: does the expected line look reasonable?
                let line_idx = line_number - 1;
                let line_content = lines[line_idx];

                // Check if the line contains the symbol or looks like a definition
                let looks_correct = if let Some(symbol) = symbol_name {
                    line_content.contains(symbol)
                        || (symbol.contains("::")
                            && line_content.contains(symbol.split("::").last().unwrap_or("")))
                } else {
                    line_content.contains("::") || line_content.contains("(")
                };

                if looks_correct {
                    Some(line_number)
                } else if let Some(symbol) = symbol_name {
                    // Try to find the symbol nearby
                    find_symbol_in_local_content(&local_content, line_number, symbol)
                } else {
                    None
                }
            } else {
                // Line number is out of bounds, search the entire file
                if let Some(symbol) = symbol_name {
                    find_symbol_in_local_content(&local_content, 1, symbol)
                } else {
                    None
                }
            };

            let final_line = actual_line.unwrap_or(line_number);

            // Try to extract the complete method first
            let (_, method_lines) = extract_complete_method(&lines, final_line);

            // If we got a complete method, return it
            if method_lines.len() > 1 {
                return Ok(method_lines.join("\n"));
            }

            // Fallback to context-based extraction
            let start_line = if final_line > context_lines {
                final_line - context_lines
            } else {
                1
            };
            let end_line = std::cmp::min(final_line + context_lines, lines.len());

            let mut result = String::new();
            for (i, line) in lines.iter().enumerate() {
                let line_num = i + 1;
                if line_num >= start_line && line_num <= end_line {
                    let marker = if line_num == final_line { ">>>" } else { "   " };
                    result.push_str(&format!("{marker} {line_num:4}: {line}\n"));
                }
            }

            return Ok(result);
        }
    }

    // Fall back to network request
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

    let github_url =
        format!("https://raw.githubusercontent.com/{github_repo}/{branch}/{file_path}");

    let request_log = if log_requests {
        Some(log_request_start("GET", &github_url))
    } else {
        None
    };

    let response = client.get(&github_url).send().await?;

    if !response.status().is_success() {
        if let Some(req_log) = request_log {
            log_request_end(req_log, response.status().as_u16(), 0);
        }
        anyhow::bail!("Could not fetch file from GitHub: {}", response.status());
    }

    let file_content = response.text().await?;
    let response_size = file_content.len();

    if let Some(req_log) = request_log {
        log_request_end(req_log, 200, response_size);
    }
    let lines: Vec<&str> = file_content.lines().collect();

    // Try to extract the complete method first
    let (_, method_lines) = extract_complete_method(&lines, line_number);

    // If we got a complete method, return it
    if method_lines.len() > 1 {
        return Ok(method_lines.join("\n"));
    }

    // Fallback to context-based extraction
    let start_line = if line_number > context_lines {
        line_number - context_lines
    } else {
        1
    };
    let end_line = std::cmp::min(line_number + context_lines, lines.len());

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if line_num >= start_line && line_num <= end_line {
            let marker = if line_num == line_number {
                ">>>"
            } else {
                "   "
            };
            result.push_str(&format!("{marker} {line_num:4}: {line}\n"));
        }
    }

    Ok(result)
}

/// Find and display the definition of a symbol
async fn find_and_display_definition(
    client: &Client,
    repo: &str,
    symbol: &str,
    path_filter: Option<&str>,
    args: &Args,
) -> anyhow::Result<()> {
    // Step 1: Find potential definition locations from search results
    debug!("Step 1: Finding potential definition locations...");
    // Use id: prefix for better definition searches
    let query = format!("id:{symbol}");
    let file_locations =
        find_symbol_in_search_results(client, repo, &query, path_filter, args).await?;

    if file_locations.is_empty() {
        error!("No potential definitions found for '{symbol}'");
        return Ok(());
    }

    debug!(
        "Found {} potential definition location(s)",
        file_locations.len()
    );

    // Step 2: Show the definition we found directly from search results
    if let Some((file_path, line_number)) = file_locations.first() {
        match get_definition_context(
            client,
            repo,
            file_path,
            *line_number,
            10,
            Some(symbol),
            args.log_requests,
        )
        .await
        {
            Ok(context) => {
                println!("{context}");
            }
            Err(e) => {
                error!("Could not fetch context: {e}");
            }
        }

        return Ok(());
    }

    error!("No definition found for symbol '{symbol}'");
    Ok(())
}

async fn get_file(
    client: &Client,
    repo: &str,
    path: &str,
    log_requests: bool,
) -> anyhow::Result<()> {
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

    let github_url = format!("https://raw.githubusercontent.com/{github_repo}/{branch}/{path}");

    let request_log = if log_requests {
        Some(log_request_start("GET", &github_url))
    } else {
        None
    };

    let response = client.get(&github_url).send().await?;

    if response.status().is_success() {
        let text = response.text().await?;
        let response_size = text.len();

        if let Some(req_log) = request_log {
            log_request_end(req_log, 200, response_size);
        }

        print!("{text}");
        return Ok(());
    } else if let Some(req_log) = request_log {
        log_request_end(req_log, response.status().as_u16(), 0);
    }

    // fallback: provide link to Searchfox
    error!(
        "GitHub fetch failed ({}). You can try viewing it at:\nhttps://searchfox.org/{}/source/{}",
        response.status(),
        repo,
        path
    );

    anyhow::bail!("Could not fetch file from GitHub");
}

async fn search_call_graph(client: &Client, args: &Args) -> anyhow::Result<()> {
    let query = if let Some(symbol) = &args.calls_from {
        format!(
            "calls-from:'{}' depth:{} graph-format:json",
            symbol, args.depth
        )
    } else if let Some(symbol) = &args.calls_to {
        format!(
            "calls-to:'{}' depth:{} graph-format:json",
            symbol, args.depth
        )
    } else if let Some(symbols) = &args.calls_between {
        let parts: Vec<&str> = symbols.split(',').collect();
        if parts.len() == 2 {
            format!(
                "calls-between-source:'{}' calls-between-target:'{}' depth:{} graph-format:json",
                parts[0].trim(),
                parts[1].trim(),
                args.depth
            )
        } else {
            anyhow::bail!(
                "calls-between requires two symbols separated by comma, e.g., 'ClassA,ClassB'"
            );
        }
    } else {
        anyhow::bail!("No call graph query specified");
    };

    let mut url = Url::parse(&format!(
        "https://searchfox.org/{}/query/default",
        args.repo
    ))?;
    url.query_pairs_mut().append_pair("q", &query);

    let request_log = if args.log_requests {
        Some(log_request_start("GET", url.as_ref()))
    } else {
        None
    };

    let response = client
        .get(url.clone())
        .header("Accept", "application/json")
        .send()
        .await?;

    if !response.status().is_success() {
        if let Some(req_log) = request_log {
            log_request_end(req_log, response.status().as_u16(), 0);
        }
        anyhow::bail!("Request failed: {}", response.status());
    }

    let response_text = response.text().await?;
    let response_size = response_text.len();

    if let Some(req_log) = request_log {
        log_request_end(req_log, 200, response_size);
    }

    // Parse and display call graph results
    match serde_json::from_str::<serde_json::Value>(&response_text) {
        Ok(json) => {
            if let Some(symbol_graph) = json.get("SymbolGraphCollection") {
                println!("Call graph results found!");
                println!("{}", serde_json::to_string_pretty(symbol_graph)?);
            } else {
                // Fallback to display all non-metadata results
                match serde_json::from_str::<SearchfoxResponse>(&response_text) {
                    Ok(parsed_json) => {
                        let mut found_results = false;
                        for (key, value) in &parsed_json {
                            if key.starts_with('*') {
                                continue; // skip metadata
                            }

                            if value.as_array().is_some() || value.as_object().is_some() {
                                found_results = true;
                                println!("{}: {}", key, serde_json::to_string_pretty(value)?);
                            }
                        }

                        if !found_results {
                            println!("No call graph results found for the query.");
                        }
                    }
                    Err(_) => {
                        println!("Call graph results:");
                        println!("{}", serde_json::to_string_pretty(&json)?);
                    }
                }
            }
        }
        Err(_) => {
            println!("Failed to parse response as JSON");
            println!("Raw response:");
            println!("{}", response_text);
        }
    }

    Ok(())
}

async fn search_code(client: &Client, args: &Args) -> anyhow::Result<()> {
    // Build query string based on arguments
    let query = if let Some(symbol) = &args.symbol {
        format!("symbol:{symbol}")
    } else if let Some(id) = &args.id {
        format!("id:{id}")
    } else if let Some(q) = &args.query {
        // Check if query already contains advanced syntax
        if q.contains("path:")
            || q.contains("pathre:")
            || q.contains("symbol:")
            || q.contains("id:")
            || q.contains("text:")
            || q.contains("re:")
        {
            q.clone()
        } else {
            // Add context if specified
            if let Some(context) = args.context {
                format!("context:{context} text:{q}")
            } else {
                q.clone()
            }
        }
    } else if args.path.is_some() {
        // If only path is specified, use an empty query
        String::new()
    } else {
        anyhow::bail!("No query specified");
    };

    let mut url = Url::parse(&format!("https://searchfox.org/{}/search", args.repo))?;
    url.query_pairs_mut()
        .append_pair("q", &query)
        .append_pair("case", if args.case { "true" } else { "false" })
        .append_pair("regexp", if args.regexp { "true" } else { "false" });
    if let Some(path) = &args.path {
        url.query_pairs_mut().append_pair("path", path);
    }

    let request_log = if args.log_requests {
        Some(log_request_start("GET", url.as_ref()))
    } else {
        None
    };

    let response = client
        .get(url.clone())
        .header("Accept", "application/json")
        .send()
        .await?;

    if !response.status().is_success() {
        if let Some(req_log) = request_log {
            log_request_end(req_log, response.status().as_u16(), 0);
        }
        anyhow::bail!("Request failed: {}", response.status());
    }

    let response_text = response.text().await?;
    let response_size = response_text.len();

    if let Some(req_log) = request_log {
        log_request_end(req_log, 200, response_size);
    }

    let json: SearchfoxResponse = serde_json::from_str(&response_text)?;

    let mut count = 0;

    for (key, value) in &json {
        if key.starts_with('*') {
            continue; // skip metadata
        }

        if let Some(files_array) = value.as_array() {
            for file in files_array {
                let file: File = serde_json::from_value(file.clone())?;

                // Skip files that don't match language filters
                if !matches_language_filter(&file.path, args) {
                    continue;
                }

                // If searching by path only, show the file path even if there are no line matches
                if args.path.is_some()
                    && args.query.is_none()
                    && args.symbol.is_none()
                    && args.id.is_none()
                {
                    if count >= args.limit {
                        break;
                    }
                    println!("{}", file.path);
                    count += 1;
                } else {
                    for line in file.lines {
                        if count >= args.limit {
                            break;
                        }
                        println!("{}:{}: {}", file.path, line.lno, line.line.trim_end());
                        count += 1;
                    }
                }
            }
        } else if let Some(obj) = value.as_object() {
            for (_category, file_list) in obj {
                if let Some(files) = file_list.as_array() {
                    for file in files {
                        let file: File = serde_json::from_value(file.clone())?;

                        // Skip files that don't match language filters
                        if !matches_language_filter(&file.path, args) {
                            continue;
                        }

                        // If searching by path only, show the file path even if there are no line matches
                        if args.path.is_some()
                            && args.query.is_none()
                            && args.symbol.is_none()
                            && args.id.is_none()
                        {
                            if count >= args.limit {
                                break;
                            }
                            println!("{}", file.path);
                            count += 1;
                        } else {
                            for line in file.lines {
                                if count >= args.limit {
                                    break;
                                }
                                println!("{}:{}: {}", file.path, line.lno, line.line.trim_end());
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        if count >= args.limit {
            break;
        }
    }

    println!("Total matches: {count}");
    Ok(())
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut builder = env_logger::Builder::from_default_env();
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Error);
    }
    builder.init();
    let args = Args::parse();

    let client = create_tls13_client()?;

    // Perform initial ping if request logging is enabled
    if args.log_requests {
        eprintln!("=== REQUEST LOGGING ENABLED ===");
        if let Err(e) = ping_searchfox(&client, &args.repo).await {
            eprintln!("[PING] Warning: Could not ping searchfox.org: {e}");
        }
        eprintln!("================================");
    }

    if let Some(symbol) = &args.define {
        find_and_display_definition(&client, &args.repo, symbol, args.path.as_deref(), &args).await
    } else if let Some(path) = &args.get_file {
        get_file(&client, &args.repo, path, args.log_requests).await
    } else if args.calls_from.is_some() || args.calls_to.is_some() || args.calls_between.is_some() {
        search_call_graph(&client, &args).await
    } else if args.query.is_some()
        || args.symbol.is_some()
        || args.id.is_some()
        || args.path.is_some()
    {
        search_code(&client, &args).await
    } else {
        anyhow::bail!(
            "Either --query, --symbol, --id, --get-file, --define, --calls-from, --calls-to, --calls-between, or --path must be provided"
        );
    }
}
