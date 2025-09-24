use anyhow::Result;
use clap::Parser;
use log::error;
use searchfox_lib::{
    call_graph::CallGraphQuery, search::SearchOptions, SearchfoxClient,
};

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

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = env_logger::Builder::from_default_env();
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Error);
    }
    builder.init();
    let args = Args::parse();

    let client = SearchfoxClient::new(args.repo.clone(), args.log_requests)?;

    if args.log_requests {
        eprintln!("=== REQUEST LOGGING ENABLED ===");
        if let Err(e) = client.ping().await {
            eprintln!("[PING] Warning: Could not ping searchfox.org: {e}");
        }
        eprintln!("================================");
    }

    let search_options = SearchOptions {
        query: args.query.clone(),
        path: args.path.clone(),
        case: args.case,
        regexp: args.regexp,
        limit: args.limit,
        context: args.context,
        symbol: args.symbol.clone(),
        id: args.id.clone(),
        cpp: args.cpp,
        c_lang: args.c_lang,
        webidl: args.webidl,
        js: args.js,
    };

    if let Some(symbol) = &args.define {
        let result = client
            .find_and_display_definition(symbol, args.path.as_deref(), &search_options)
            .await?;
        if !result.is_empty() {
            println!("{}", result);
        }
    } else if let Some(path) = &args.get_file {
        let content = client.get_file(path).await?;
        print!("{}", content);
    } else if args.calls_from.is_some() || args.calls_to.is_some() || args.calls_between.is_some()
    {
        let query = CallGraphQuery {
            calls_from: args.calls_from,
            calls_to: args.calls_to,
            calls_between: args.calls_between.map(|s| {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 2 {
                    (parts[0].trim().to_string(), parts[1].trim().to_string())
                } else {
                    (s.clone(), String::new())
                }
            }),
            depth: args.depth,
        };

        let result = client.search_call_graph(&query).await?;
        if result.as_object().map_or(false, |o| !o.is_empty())
            || result.as_array().map_or(false, |a| !a.is_empty())
        {
            println!("Call graph results found!");
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("No call graph results found for the query.");
        }
    } else if args.query.is_some()
        || args.symbol.is_some()
        || args.id.is_some()
        || args.path.is_some()
    {
        let results = client.search(&search_options).await?;
        let mut count = 0;
        for result in &results {
            if result.line_number == 0 {
                println!("{}", result.path);
            } else {
                println!("{}:{}: {}", result.path, result.line_number, result.line);
            }
            count += 1;
        }
        println!("Total matches: {count}");
    } else {
        error!(
            "Either --query, --symbol, --id, --get-file, --define, --calls-from, --calls-to, --calls-between, or --path must be provided"
        );
        std::process::exit(1);
    }

    Ok(())
}