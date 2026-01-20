use anyhow::Result;
use clap::Parser;
use log::error;
use searchfox_lib::{
    call_graph::{format_call_graph_markdown, CallGraphQuery},
    parse_commit_header,
    search::SearchOptions,
    CategoryFilter, SearchfoxClient,
};
use std::collections::HashMap;

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
        help = "Line range for --get-file (e.g., 10-20, 10, 10-)",
        long_help = "Specify line range when using --get-file.\nFormats:\n  --lines 10-20  (lines 10 through 20)\n  --lines 10     (just line 10)\n  --lines 10-    (from line 10 to end)\n  --lines -20    (from start to line 20)\nExample: --get-file dom/media/AudioStream.h --lines 100-150"
    )]
    lines: Option<String>,

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
        default_value_t = 1,
        help = "Set traversal depth for call graph searches",
        long_help = "Set the depth of traversal for call graph searches. Higher values show more indirect calls.\nDefault is 1. Example: --depth 3"
    )]
    depth: u32,

    #[arg(
        long = "exclude-tests",
        help = "Exclude test files from results",
        conflicts_with_all = ["only_tests", "only_generated", "only_normal"]
    )]
    exclude_tests: bool,

    #[arg(
        long = "exclude-generated",
        help = "Exclude generated files from results",
        conflicts_with_all = ["only_tests", "only_generated", "only_normal"]
    )]
    exclude_generated: bool,

    #[arg(
        long = "only-tests",
        help = "Show only test files",
        conflicts_with_all = ["exclude_tests", "exclude_generated", "only_generated", "only_normal"]
    )]
    only_tests: bool,

    #[arg(
        long = "only-generated",
        help = "Show only generated files",
        conflicts_with_all = ["exclude_tests", "exclude_generated", "only_tests", "only_normal"]
    )]
    only_generated: bool,

    #[arg(
        long = "only-normal",
        help = "Show only normal (non-test, non-generated) files",
        conflicts_with_all = ["exclude_tests", "exclude_generated", "only_tests", "only_generated"]
    )]
    only_normal: bool,

    #[arg(
        long = "blame",
        default_value_t = false,
        help = "Show blame/history info for results",
        long_help = "Augment query results with blame information.\nShows commit hash, author, date, and bug for each result line.\nWorks with --define, --symbol, -q, --get-file, etc."
    )]
    blame: bool,
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

    let category_filter = if args.only_tests {
        CategoryFilter::OnlyTests
    } else if args.only_generated {
        CategoryFilter::OnlyGenerated
    } else if args.only_normal {
        CategoryFilter::OnlyNormal
    } else if args.exclude_tests && args.exclude_generated {
        CategoryFilter::ExcludeTestsAndGenerated
    } else if args.exclude_tests {
        CategoryFilter::ExcludeTests
    } else if args.exclude_generated {
        CategoryFilter::ExcludeGenerated
    } else {
        CategoryFilter::All
    };

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
        category_filter,
    };

    if let Some(symbol) = &args.define {
        let result = client
            .find_and_display_definition(symbol, args.path.as_deref(), &search_options)
            .await?;
        if !result.is_empty() {
            if args.blame {
                // First, get the file path from the result by finding the symbol location
                let file_locations = client
                    .find_symbol_locations(symbol, args.path.as_deref(), &search_options)
                    .await?;

                if let Some((file_path, _)) = file_locations.first() {
                    // Parse the result to extract line numbers
                    let line_numbers = extract_line_numbers_from_definition(&result);

                    if !line_numbers.is_empty() {
                        // Fetch blame info
                        let blame_map =
                            client.get_blame_for_lines(file_path, &line_numbers).await?;

                        // Output with grouped blame
                        print_definition_with_grouped_blame(&result, &blame_map);
                    } else {
                        println!("{}", result);
                    }
                } else {
                    println!("{}", result);
                }
            } else {
                println!("{}", result);
            }
        }
    } else if let Some(path) = &args.get_file {
        let content = client.get_file(path).await?;

        // Parse line range if provided
        let (start_line, end_line) = if let Some(ref range) = args.lines {
            parse_line_range(range, content.lines().count())?
        } else {
            (1, content.lines().count())
        };

        // Filter content to the specified range
        let filtered_lines: Vec<(usize, &str)> = content
            .lines()
            .enumerate()
            .map(|(idx, line)| (idx + 1, line))
            .filter(|(line_num, _)| *line_num >= start_line && *line_num <= end_line)
            .collect();

        if args.blame {
            // Get line numbers for the filtered range
            let line_numbers: Vec<usize> = filtered_lines.iter().map(|(num, _)| *num).collect();

            // Fetch blame for the range
            let blame_map = client.get_blame_for_lines(path, &line_numbers).await?;

            // Format content with line numbers (using consistent spacing like --define)
            let mut formatted_content = String::new();
            for (line_num, line) in filtered_lines {
                formatted_content.push_str(&format!("    {:4}: {}\n", line_num, line));
            }

            // Print with grouped blame
            print_definition_with_grouped_blame(&formatted_content, &blame_map);
        } else {
            for (line_num, line) in filtered_lines {
                if args.lines.is_some() {
                    // Show line numbers when range is specified
                    println!("{:4}: {}", line_num, line);
                } else {
                    // Original behavior without line numbers
                    println!("{}", line);
                }
            }
        }
    } else if args.calls_from.is_some() || args.calls_to.is_some() || args.calls_between.is_some() {
        let query_text = if let Some(ref symbol) = args.calls_from {
            format!("calls-from:'{}' depth:{}", symbol, args.depth)
        } else if let Some(ref symbol) = args.calls_to {
            format!("calls-to:'{}' depth:{}", symbol, args.depth)
        } else if let Some(ref between) = args.calls_between {
            let parts: Vec<&str> = between.split(',').collect();
            if parts.len() == 2 {
                format!(
                    "calls-between-source:'{}' calls-between-target:'{}' depth:{}",
                    parts[0].trim(),
                    parts[1].trim(),
                    args.depth
                )
            } else {
                format!("calls-between:'{}' depth:{}", between, args.depth)
            }
        } else {
            String::from("call-graph query")
        };

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
        if result.as_object().is_some_and(|o| !o.is_empty())
            || result.as_array().is_some_and(|a| !a.is_empty())
        {
            if std::env::var("DEBUG_JSON").is_ok() {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                let markdown = format_call_graph_markdown(&query_text, &result);
                print!("{}", markdown);
            }
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

        if args.blame {
            // Group results by file for efficient blame fetching
            let mut results_by_file: HashMap<String, Vec<(usize, String)>> = HashMap::new();
            for result in &results {
                if result.line_number > 0 {
                    results_by_file
                        .entry(result.path.clone())
                        .or_default()
                        .push((result.line_number, result.line.clone()));
                }
            }

            // Fetch and display results with blame
            for (path, lines) in results_by_file {
                let line_numbers: Vec<usize> = lines.iter().map(|(ln, _)| *ln).collect();
                let blame_map = client.get_blame_for_lines(&path, &line_numbers).await?;

                for (line_number, line_text) in lines {
                    println!("{}:{}: {}", path, line_number, line_text);

                    if let Some(blame_info) = blame_map.get(&line_number) {
                        if let Some(ref commit_info) = blame_info.commit_info {
                            let parsed = parse_commit_header(&commit_info.header);
                            let short_hash = &blame_info.commit_hash[..8];
                            if let Some(bug) = parsed.bug_number {
                                println!(
                                    "  [{}] Bug {}: {} ({}, {})",
                                    short_hash, bug, parsed.message, parsed.author, parsed.date
                                );
                            } else {
                                println!(
                                    "  [{}] {} ({}, {})",
                                    short_hash, parsed.message, parsed.author, parsed.date
                                );
                            }
                        }
                    }
                    count += 1;
                }
            }
        } else {
            // Original output without blame
            for result in &results {
                if result.line_number == 0 {
                    println!("{}", result.path);
                } else {
                    println!("{}:{}: {}", result.path, result.line_number, result.line);
                }
                count += 1;
            }
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

/// Extract line numbers from definition output
/// Lines are formatted like ">>> 469: code" or "   470: code"
fn extract_line_numbers_from_definition(output: &str) -> Vec<usize> {
    output
        .lines()
        .filter_map(|line| {
            // Look for pattern like ">>> 469:" or "   470:"
            let trimmed = line.trim_start();
            if trimmed.starts_with(">>>") || line.starts_with("   ") {
                // Extract the number after the marker
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 2 {
                    let num_part = parts[0].trim_start_matches(">>>").trim();
                    num_part.parse::<usize>().ok()
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Parse line number from a formatted output line
/// Returns Some(line_number) if the line contains a line number marker
fn parse_line_number_from_output(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.starts_with(">>>") || line.starts_with("   ") {
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() >= 2 {
            let num_part = parts[0].trim_start_matches(">>>").trim();
            num_part.parse::<usize>().ok()
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse line range string (e.g., "10-20", "10", "10-", "-20")
/// Returns (start_line, end_line) inclusive
fn parse_line_range(range: &str, total_lines: usize) -> Result<(usize, usize)> {
    let range = range.trim();

    if range.contains('-') {
        let parts: Vec<&str> = range.split('-').collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid line range format: '{}'. Expected formats: 10-20, 10, 10-, -20",
                range
            );
        }

        let start = if parts[0].is_empty() {
            1
        } else {
            parts[0]
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("Invalid start line number: '{}'", parts[0]))?
        };

        let end = if parts[1].is_empty() {
            total_lines
        } else {
            parts[1]
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("Invalid end line number: '{}'", parts[1]))?
        };

        if start < 1 {
            anyhow::bail!("Start line must be >= 1");
        }
        if end > total_lines {
            anyhow::bail!("End line {} exceeds file length {}", end, total_lines);
        }
        if start > end {
            anyhow::bail!("Start line {} is greater than end line {}", start, end);
        }

        Ok((start, end))
    } else {
        // Single line number
        let line_num = range
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid line number: '{}'", range))?;

        if line_num < 1 {
            anyhow::bail!("Line number must be >= 1");
        }
        if line_num > total_lines {
            anyhow::bail!("Line {} exceeds file length {}", line_num, total_lines);
        }

        Ok((line_num, line_num))
    }
}

/// Print definition with blame info, grouping consecutive lines with the same commit
fn print_definition_with_grouped_blame(
    definition: &str,
    blame_map: &HashMap<usize, searchfox_lib::BlameInfo>,
) {
    #[derive(Clone)]
    struct CommitRange {
        start_line: usize,
        end_line: usize,
        commit_hash: String,
        message: String,
    }

    let mut current_range: Option<CommitRange> = None;
    let mut pending_output: Vec<String> = Vec::new();

    for line in definition.lines() {
        pending_output.push(line.to_string());

        if let Some(line_num) = parse_line_number_from_output(line) {
            if let Some(blame_info) = blame_map.get(&line_num) {
                if let Some(ref commit_info) = blame_info.commit_info {
                    let parsed = parse_commit_header(&commit_info.header);
                    let short_hash = blame_info.commit_hash[..8].to_string();

                    let message = if let Some(bug) = parsed.bug_number {
                        format!("Bug {}: {}", bug, parsed.message)
                    } else {
                        parsed.message.clone()
                    };

                    // Check if this is the same commit as current range
                    if let Some(ref range) = current_range {
                        if range.commit_hash == short_hash {
                            // Extend current range
                            current_range = Some(CommitRange {
                                start_line: range.start_line,
                                end_line: line_num,
                                commit_hash: short_hash,
                                message,
                            });
                            continue;
                        } else {
                            // Different commit - flush current range
                            for output_line in &pending_output[..pending_output.len() - 1] {
                                println!("{}", output_line);
                            }
                            pending_output.clear();
                            pending_output.push(line.to_string());

                            // Print blame for previous range
                            if range.start_line == range.end_line {
                                println!(
                                    "         [{}] {} (line {})",
                                    range.commit_hash, range.message, range.start_line
                                );
                            } else {
                                println!(
                                    "         [{}] {} (lines {}-{})",
                                    range.commit_hash,
                                    range.message,
                                    range.start_line,
                                    range.end_line
                                );
                            }
                        }
                    }

                    // Start new range
                    current_range = Some(CommitRange {
                        start_line: line_num,
                        end_line: line_num,
                        commit_hash: short_hash,
                        message,
                    });
                }
            }
        }
    }

    // Flush remaining output
    for output_line in &pending_output {
        println!("{}", output_line);
    }

    // Print final range if exists
    if let Some(range) = current_range {
        if range.start_line == range.end_line {
            println!(
                "         [{}] {} (line {})",
                range.commit_hash, range.message, range.start_line
            );
        } else {
            println!(
                "         [{}] {} (lines {}-{})",
                range.commit_hash, range.message, range.start_line, range.end_line
            );
        }
    }
}
