# Searchfox Python Examples

This directory contains example scripts demonstrating how to use the searchfox Python bindings.

## Setup

Before running these examples, make sure to build and install the searchfox Python package:

```bash
# From the repository root
pip install maturin
maturin develop
```

## Examples

### 1. demo.py - Basic API Demo

Comprehensive demonstration of all searchfox API features:

```bash
python python/examples/demo.py

# Run with advanced examples
python python/examples/demo.py --advanced
```

Features demonstrated:
- Basic search queries
- Language-filtered searches (C++, C, JavaScript, WebIDL)
- Symbol and identifier searches
- File retrieval
- Definition finding
- Call graph analysis
- Repository switching
- Regex searches
- Performance monitoring

### 2. code_analyzer.py - Practical Code Analysis

A more practical example showing how to analyze code patterns and relationships:

```bash
python python/examples/code_analyzer.py
```

Features:
- Find class implementations and inheritance hierarchies
- Analyze include patterns in directories
- Track method calls across the codebase
- Build class hierarchy trees
- Analyze API usage patterns
- Security pattern detection

## Best Practices

**Important:** Avoid expensive full-text searches. Use indexed searches instead:

- ✅ **Good:** `search(symbol="AudioContext")` - Uses searchfox's index
- ✅ **Good:** `search(id="main")` - Uses searchfox's index
- ✅ **Good:** `get_definition("AudioContext::CreateGain")` - Uses index internally
- ❌ **Bad:** `search(query="AudioContext")` - Expensive full-text search
- ❌ **Bad:** `search(query="path:dom/media text")` - Still expensive

For text searches in local code, use ripgrep via subprocess:
```python
import subprocess
result = subprocess.run(['rg', 'pattern'], capture_output=True, text=True)
```

## Quick Start Examples

### Simple Indexed Search

```python
import searchfox

# Use indexed symbol search (efficient)
results = searchfox.search(symbol="AudioContext", limit=5)
for path, line_num, line in results:
    print(f"{path}:{line_num}: {line}")
```

### Get a File

```python
import searchfox

# Get file contents
content = searchfox.get_file("dom/media/AudioStream.h")
print(content)
```

### Find Definition

```python
import searchfox

# Find symbol definition
client = searchfox.SearchfoxClient("mozilla-central")
definition = client.get_definition("AudioContext::CreateGain")
print(definition)
```

### Language-Specific Search

```python
import searchfox

# Search only in C++ files (using indexed search)
client = searchfox.SearchfoxClient("mozilla-central")
results = client.search(
    symbol="malloc",
    cpp=True,
    path="^memory",
    limit=10
)
```

### Call Graph Analysis

```python
import searchfox

client = searchfox.SearchfoxClient("mozilla-central")
call_graph = client.search_call_graph(
    calls_from="mozilla::dom::AudioContext::CreateGain",
    depth=2
)
print(call_graph)
```

## Use Cases

The searchfox Python API is useful for:

1. **Code Analysis**: Understanding code structure, dependencies, and patterns
2. **Refactoring**: Finding all usages of APIs before making changes
3. **Security Auditing**: Searching for potentially unsafe patterns
4. **Documentation**: Generating documentation from code analysis
5. **Learning**: Exploring how Mozilla implements various features
6. **Testing**: Finding test cases for specific functionality
7. **Performance Analysis**: Understanding call chains and dependencies

## Available Repositories

- `mozilla-central` - Main Firefox development (default)
- `autoland` - Integration repository
- `mozilla-beta` - Beta release branch
- `mozilla-release` - Release branch
- `mozilla-esr115` - ESR 115 branch
- `mozilla-esr128` - ESR 128 branch
- `mozilla-esr140` - ESR 140 branch
- `comm-central` - Thunderbird development

## API Reference

### SearchfoxClient

```python
class SearchfoxClient:
    def __init__(self, repo="mozilla-central", log_requests=False)

    def search(self, query=None, path=None, case=False, regexp=False,
               limit=50, context=None, symbol=None, id=None,
               cpp=False, c_lang=False, webidl=False, js=False)

    def get_file(self, path: str) -> str

    def get_definition(self, symbol: str, path_filter=None) -> str

    def search_call_graph(self, calls_from=None, calls_to=None,
                         calls_between=None, depth=2) -> str

    def ping(self) -> float
```

### Convenience Functions

```python
searchfox.search(query, repo="mozilla-central", **options)
searchfox.get_file(path, repo="mozilla-central", log_requests=False)
searchfox.get_definition(symbol, repo="mozilla-central", path_filter=None)
```