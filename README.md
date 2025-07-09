# searchfox-cli

[![Crates.io](https://img.shields.io/crates/v/searchfox-cli.svg)](https://crates.io/crates/searchfox-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Build Status](https://github.com/padenot/searchfox-cli/workflows/CI/badge.svg)](https://github.com/padenot/searchfox-cli/actions)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Rust Report Card](https://rust-reportcard.xuri.me/badge/github.com/padenot/searchfox-cli)](https://rust-reportcard.xuri.me/report/github.com/padenot/searchfox-cli)
[![Dependencies](https://deps.rs/repo/github/padenot/searchfox-cli/status.svg)](https://deps.rs/repo/github/padenot/searchfox-cli)
[![Downloads](https://img.shields.io/crates/d/searchfox-cli.svg)](https://crates.io/crates/searchfox-cli)

A command-line interface for searching Mozilla codebases using searchfox.org,
written by and for Claude Code.

## Features

- Search across multiple Mozilla repositories (mozilla-central, autoland, beta, release, ESR branches, comm-central)
- Support for regular expressions and case-sensitive search
- Filter results by file path patterns (or search for files by path alone)
- **Language filtering**: Filter results by programming language (C++, C, WebIDL, JavaScript)
- Fetch and display file contents directly from GitHub
- **Complete function and class extraction**: Intelligently extracts entire method/function bodies and class definitions with brace matching
- **Symbol search**: Uses searchfox's native symbol indexing for precise symbol lookups
- **Advanced definition finding**: Uses searchfox's structured data for reliable symbol resolution, prioritizes class definitions
- **Request logging**: Detailed HTTP request timing and performance analysis
- Configurable result limits

## Installation

```bash
cargo install --path .
```

## Usage

### Basic Search

```bash
# Search for "AudioStream" in mozilla-central
searchfox-cli -q AudioStream

# Search with case sensitivity
searchfox-cli -q AudioStream -C

# Search with regular expressions
searchfox-cli -q '^Audio.*' -r

# Limit results to 10 matches
searchfox-cli -q AudioStream -l 10
```

### Symbol and Definition Search

```bash
# Find symbol definitions
searchfox-cli --symbol AudioContext

# Find exact identifier matches (not prefix-based)
searchfox-cli --id main

# Search for symbols using searchfox's symbol index
searchfox-cli --symbol 'AudioContext'
searchfox-cli --symbol 'CreateGain'

# Search for symbols in specific paths
searchfox-cli -q 'path:dom/media symbol:AudioStream'
```

### Repository Selection

```bash
# Search in autoland repository
searchfox-cli -q AudioStream -R autoland

# Search in beta branch
searchfox-cli -q AudioStream -R mozilla-beta
```

### Path Filtering

```bash
# Search only in dom/media directory
searchfox-cli -q AudioStream -p ^dom/media

# Search in specific file patterns
searchfox-cli -q AudioStream -p '\.cpp$'

# Use -p alone to search for files by path pattern
searchfox-cli -p PContent.ipdl
searchfox-cli -p AudioContext.cpp

# Using advanced path syntax in query
searchfox-cli -q 'path:dom/media AudioStream'
searchfox-cli -q 'pathre:^dom/(media|audio) AudioStream'
```

### Language Filtering

Filter search results by programming language using language-specific flags:

```bash
# Search only in C++ files (.cc, .cpp, .h, .hh, .hpp)
searchfox-cli -q AudioContext --cpp
searchfox-cli --define AudioContext -p dom/media --cpp

# Search only in C files (.c, .h)
searchfox-cli -q malloc --c

# Search only in WebIDL files (.webidl)
searchfox-cli -q AudioContext --webidl

# Search only in JavaScript files (.js, .mjs, .ts, .cjs, .jsx, .tsx)
searchfox-cli -q AudioContext --js

# Without language filters, all file types are included
searchfox-cli --define AudioContext -p dom/media
```

### Advanced Query Features

```bash
# Search with context lines
searchfox-cli -q AudioStream --context 3

# Text search with regex
searchfox-cli -q 're:AudioContext::.*Create'

# Exact text search (escapes regex chars)
searchfox-cli -q 'text:function main()'

# Combined advanced queries
searchfox-cli -q 'context:3 pathre:dom/media symbol:AudioStream'
```

### Symbol Search

The `--symbol` flag uses searchfox's native symbol indexing for precise symbol lookups:

```bash
# Search for symbols by name
searchfox-cli --symbol 'AudioContext'
searchfox-cli --symbol 'CreateGain'

# Combine with path filtering
searchfox-cli -q 'path:dom/media symbol:AudioStream'
```

Symbol search relies on searchfox's own symbol database, which includes properly mangled C++ symbols and other language constructs as indexed by the searchfox infrastructure.

### Advanced Definition Finding

The `--define` flag provides an advanced way to find symbol definitions by:

1. **Symbol Search**: Uses `id:` prefix internally for precise symbol lookups
2. **Class/Struct Priority**: Prioritizes class and struct definitions over constructors
3. **Definition Resolution**: Searches both "Definitions" and "Declarations" categories (for C++ classes)
4. **Context Extraction**: Fetches the source file and displays the complete method/function/class

```bash
# Find class definition with full body
searchfox-cli --define AudioContext -p dom/media

# Find method definition with full context
searchfox-cli --define 'AudioContext::CreateGain'

# Filter by language
searchfox-cli --define AudioContext -p dom/media --cpp

# The tool will:
# 1. Search using id:AudioContext for precise matches
# 2. Prioritize class definitions over constructor declarations
# 3. Extract the complete class body (with brace matching)
# 4. Display the full definition with proper highlighting
```

This approach leverages searchfox's comprehensive symbol database for reliable definition finding.

#### Example Output:

**For class definitions:**
```bash
$ searchfox-cli --define AudioContext -p dom/media --cpp
>>>  135: class AudioContext final : public DOMEventTargetHelper,
     136:                            public nsIMemoryReporter,
     137:                            public RelativeTimeline {
     138:   AudioContext(nsPIDOMWindowInner* aParentWindow, bool aIsOffline,
     139:                uint32_t aNumberOfChannels = 0, uint32_t aLength = 0,
     140:                float aSampleRate = 0.0f);
     141:   ~AudioContext();
     142: 
     143:  public:
     144:   typedef uint64_t AudioContextId;
     145: 
     146:   NS_DECL_ISUPPORTS_INHERITED
     147:   NS_DECL_CYCLE_COLLECTION_CLASS_INHERITED(AudioContext, DOMEventTargetHelper)
     148:   MOZ_DEFINE_MALLOC_SIZE_OF(MallocSizeOf)
     ...
     335:   void RegisterNode(AudioNode* aNode);
   ...  : (method too long, truncated)
```

**For method definitions:**
```bash
$ searchfox-cli --define 'AudioContext::CreateGain'
>>>  469: already_AddRefed<GainNode> AudioContext::CreateGain(ErrorResult& aRv) {
     470:   return GainNode::Create(*this, GainOptions(), aRv);
     471: }
```

The tool automatically:
- Searches searchfox's structured data for definition entries
- Uses searchfox's native symbol indexing for accurate results
- Finds the actual source file location (not generated binding files)
- Fetches the source code and displays complete methods/functions
- Highlights the exact definition line with `>>>`

#### Complete Function and Class Extraction

When using `--define`, the tool automatically detects and extracts complete function/method bodies and class definitions using intelligent brace matching:

**For simple functions:**
```bash
$ searchfox-cli --define 'AudioContext::CreateGain'
>>>  469: already_AddRefed<GainNode> AudioContext::CreateGain(ErrorResult& aRv) {
     470:   return GainNode::Create(*this, GainOptions(), aRv);
     471: }
```

**For complex constructors with initializer lists:**
```bash
$ searchfox-cli --define 'AudioContext::AudioContext'
>>>  154: AudioContext::AudioContext(nsPIDOMWindowInner* aWindow, bool aIsOffline,
     155:                            uint32_t aNumberOfChannels, uint32_t aLength,
     156:                            float aSampleRate)
     157:     : DOMEventTargetHelper(aWindow),
     158:       mId(gAudioContextId++),
     159:       mSampleRate(GetSampleRateForAudioContext(
     160:           aIsOffline, aSampleRate,
     161:           aWindow->AsGlobal()->ShouldResistFingerprinting(
     162:               RFPTarget::AudioSampleRate))),
     163:       mAudioContextState(AudioContextState::Suspended),
     164:       ...
     179:       mSuspendedByChrome(nsGlobalWindowInner::Cast(aWindow)->IsSuspended()) {
     180:   bool mute = aWindow->AddAudioContext(this);
     181:   // ... full method body continues ...
     205: }
```

**Features of complete extraction:**
- **Multi-language support**: Handles C++, Rust, and JavaScript function syntax
- **Class and struct definitions**: Extracts complete class/struct bodies with proper brace matching  
- **Smart brace matching**: Ignores braces in strings, comments, and character literals
- **Complex signatures**: Handles multi-line function signatures and initializer lists
- **Constructor support**: Properly extracts C++ constructors with member initialization lists
- **Class termination**: Handles classes/structs ending with semicolons correctly
- **Safety limits**: Truncates extremely long definitions (>200 lines) to prevent output overflow
- **Accurate parsing**: Correctly handles nested braces, escape sequences, and comment blocks

### Request Logging and Performance Analysis

The `--log-requests` flag enables comprehensive HTTP request logging for performance analysis and infrastructure planning:

```bash
# Enable detailed request logging
searchfox-cli --log-requests --define 'AudioContext::CreateGain'
```

**Features:**
- **Baseline latency measurement**: HTTP HEAD request to searchfox.org for baseline timing
- **Request tracking**: Each HTTP request gets a unique ID for correlation  
- **Detailed timing**: Start/end timestamps with duration in milliseconds
- **Response metrics**: HTTP status codes and response size in bytes
- **Performance insights**: Compare request times against baseline to identify bottlenecks

**Example output:**
```bash
=== REQUEST LOGGING ENABLED ===
[PING] Testing network latency to searchfox.org (ICMP ping disabled, using HTTP HEAD)...
[PING] HTTP HEAD latency: 573ms (HTTP 200 OK)
[PING] Note: This includes minimal server processing time, not just network latency
================================

[REQ-1] GET https://searchfox.org/mozilla-central/search?q=AudioContext%3A%3ACreateGain - START
[REQ-1] GET https://searchfox.org/mozilla-central/search?q=AudioContext%3A%3ACreateGain - END (573ms, 1188 bytes, HTTP 200)
[REQ-2] GET https://raw.githubusercontent.com/mozilla/firefox/main/dom/media/webaudio/AudioContext.cpp - START  
[REQ-2] GET https://raw.githubusercontent.com/mozilla/firefox/main/dom/media/webaudio/AudioContext.cpp - END (172ms, 44915 bytes, HTTP 200)
```

**Performance analysis insights:**
- **Network vs. server processing**: Compare request duration against baseline latency
- **Service comparison**: GitHub file fetching vs. searchfox API performance
- **Infrastructure planning**: Determine if searchfox server upgrades would help vs. network optimization
- **Request patterns**: Track multiple requests to identify consistency and variation

### File Retrieval

```bash
# Fetch and display a specific file
searchfox-cli --get-file dom/media/AudioStream.h
```

## Available Repositories

- `mozilla-central` (default) - Main Firefox development
- `autoland` - Integration repository
- `mozilla-beta` - Beta release branch
- `mozilla-release` - Release branch
- `mozilla-esr115` - ESR 115 branch
- `mozilla-esr128` - ESR 128 branch
- `mozilla-esr140` - ESR 140 branch
- `comm-central` - Thunderbird development

## Command Line Options

- `-q, --query <QUERY>` - Search query string (supports advanced syntax)
- `-R, --repo <REPO>` - Repository to search in (default: mozilla-central)
- `-p, --path <PATH>` - Filter results by path prefix using regex, or search for files by path pattern
- `-C, --case` - Enable case-sensitive search
- `-r, --regexp` - Enable regular expression search
- `-l, --limit <LIMIT>` - Maximum number of results to display (default: 50)
- `--get-file <FILE>` - Fetch and display contents of a specific file
- `--symbol <SYMBOL>` - Search for symbol definitions using searchfox's symbol index
- `--id <IDENTIFIER>` - Search for exact identifier matches
- `--context <N>` - Show N lines of context around matches
- `--define <SYMBOL>` - Find and display the definition of a symbol with full context
- `--log-requests` - Enable detailed HTTP request logging with timing and size information
- `--cpp` - Filter results to C++ files only (.cc, .cpp, .h, .hh, .hpp)
- `--c` - Filter results to C files only (.c, .h)
- `--webidl` - Filter results to WebIDL files only (.webidl)
- `--js` - Filter results to JavaScript files only (.js, .mjs, .ts, .cjs, .jsx, .tsx)

## Examples

```bash
# Find all AudioStream references
searchfox-cli -q AudioStream

# Find function definitions starting with "Audio"
searchfox-cli -q '^Audio.*' -r

# Search only in media-related files
searchfox-cli -q AudioStream -p ^dom/media

# Get a specific file
searchfox-cli --get-file dom/media/AudioStream.h

# Search in Thunderbird codebase
searchfox-cli -q "MailServices" -R comm-central

# Find where AudioContext is defined
searchfox-cli --symbol AudioContext

# Find exact matches for "main" function
searchfox-cli --id main

# Search with context lines
searchfox-cli -q AudioStream --context 5

# Symbol search using searchfox's symbol index
searchfox-cli --symbol 'AudioContext'
searchfox-cli --symbol 'CreateGain'

# Find complete definition with context
searchfox-cli --define 'AudioContext::CreateGain'
searchfox-cli --define 'AudioContext'

# Language filtering
searchfox-cli --define AudioContext -p dom/media --cpp
searchfox-cli -q malloc --c
searchfox-cli -q AudioContext --js

# File path search
searchfox-cli -p PContent.ipdl
searchfox-cli -p AudioContext.cpp

# Advanced query syntax
searchfox-cli -q 'path:dom/media symbol:AudioStream'
searchfox-cli -q 're:AudioContext::.*Create'

# Performance analysis with request logging
searchfox-cli --log-requests --define 'AudioContext::CreateGain'
searchfox-cli --log-requests -q AudioStream -l 10
```

## Dependencies

- [clap](https://crates.io/crates/clap) - Command line argument parsing
- [reqwest](https://crates.io/crates/reqwest) - HTTP client
- [tokio](https://crates.io/crates/tokio) - Async runtime
- [serde](https://crates.io/crates/serde) - Serialization/deserialization
- [anyhow](https://crates.io/crates/anyhow) - Error handling

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
