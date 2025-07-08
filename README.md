# searchfox-cli

A command-line interface for searching Mozilla codebases using searchfox.org,
written by and for Claude Code.

## Features

- Search across multiple Mozilla repositories (mozilla-central, autoland, beta, release, ESR branches, comm-central)
- Support for regular expressions and case-sensitive search
- Filter results by file path patterns
- Fetch and display file contents directly from GitHub
- **Complete function extraction**: Intelligently extracts entire method/function bodies with brace matching
- **Symbol search**: Uses searchfox's native symbol indexing for precise symbol lookups
- **Advanced definition finding**: Uses searchfox's structured data for reliable symbol resolution
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

# Using advanced path syntax in query
searchfox-cli -q 'path:dom/media AudioStream'
searchfox-cli -q 'pathre:^dom/(media|audio) AudioStream'
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

1. **Symbol Search**: Searches for the symbol using searchfox's structured data
2. **Definition Resolution**: Locates actual definitions from search results
3. **Context Extraction**: Fetches the source file and displays the complete method/function

```bash
# Find method definition with full context
searchfox-cli --define 'AudioContext::CreateGain'

# The tool will:
# 1. Search for the symbol in searchfox's structured data
# 2. Locate definition entries from search results
# 3. Fetch the source file and extract the complete method
# 4. Display the full method/function body with context
```

This approach leverages searchfox's comprehensive symbol database for reliable definition finding.

#### Example Output:

```bash
$ searchfox-cli --define 'AudioContext::CreateGain'
Searching for definition of: AudioContext::CreateGain
Step 1: Finding potential definition locations...
Analyzing search results...
Found 1 potential definition location(s)

=== DEFINITION FOUND ===
File: dom/media/webaudio/AudioContext.cpp
Line: 469

Definition context:
     459: 
     460: already_AddRefed<MediaStreamTrackAudioSourceNode>
     461: AudioContext::CreateMediaStreamTrackSource(MediaStreamTrack& aMediaStreamTrack,
     462:                                            ErrorResult& aRv) {
     463:   MediaStreamTrackAudioSourceOptions options;
     464:   options.mMediaStreamTrack = aMediaStreamTrack;
     465: 
     466:   return MediaStreamTrackAudioSourceNode::Create(*this, options, aRv);
     467: }
     468: 
>>>  469: already_AddRefed<GainNode> AudioContext::CreateGain(ErrorResult& aRv) {
     470:   return GainNode::Create(*this, GainOptions(), aRv);
     471: }
     472: 
     473: already_AddRefed<WaveShaperNode> AudioContext::CreateWaveShaper(
     474:     ErrorResult& aRv) {
     475:   return WaveShaperNode::Create(*this, WaveShaperOptions(), aRv);
     476: }
```

The tool automatically:
- Searches searchfox's structured data for definition entries
- Uses searchfox's native symbol indexing for accurate results
- Finds the actual source file location (not generated binding files)
- Fetches the source code and displays complete methods/functions
- Highlights the exact definition line with `>>>`

#### Complete Function Extraction

When using `--define`, the tool automatically detects and extracts complete function/method bodies using intelligent brace matching:

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

**Features of complete function extraction:**
- **Multi-language support**: Handles C++, Rust, and JavaScript function syntax
- **Smart brace matching**: Ignores braces in strings, comments, and character literals
- **Complex signatures**: Handles multi-line function signatures and initializer lists
- **Constructor support**: Properly extracts C++ constructors with member initialization lists
- **Safety limits**: Truncates extremely long methods (>200 lines) to prevent output overflow
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
- `-p, --path <PATH>` - Filter results by path prefix using regex
- `-C, --case` - Enable case-sensitive search
- `-r, --regexp` - Enable regular expression search
- `-l, --limit <LIMIT>` - Maximum number of results to display (default: 50)
- `--get-file <FILE>` - Fetch and display contents of a specific file
- `--symbol <SYMBOL>` - Search for symbol definitions using searchfox's symbol index
- `--id <IDENTIFIER>` - Search for exact identifier matches
- `--context <N>` - Show N lines of context around matches
- `--define <SYMBOL>` - Find and display the definition of a symbol with full context
- `--log-requests` - Enable detailed HTTP request logging with timing and size information

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
searchfox-cli --define 'CreateGain'

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
