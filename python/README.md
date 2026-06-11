# searchfox Python bindings

Python bindings for the [searchfox.org](https://searchfox.org) code search API.

## Install

```
pip install searchfox
```

For development (rebuilds the Rust extension in-place):
```
VIRTUAL_ENV=/path/to/venv just develop   # from repo root
```

## Usage

Two clients are available: `SearchfoxClient` (blocking) and `AsyncSearchfoxClient` (asyncio-native).

```python
from searchfox import SearchfoxClient, AsyncSearchfoxClient

# Synchronous
client = SearchfoxClient()                          # defaults to mozilla-central
# Asynchronous
client = AsyncSearchfoxClient()
```

All methods on `AsyncSearchfoxClient` return awaitables; the API is otherwise identical.

## API

### search

```python
results = client.search(
    query="AudioContext",   # free-text or regex
    id="AudioContext",      # exact identifier
    symbol="...",           # mangled symbol
    path="^dom/media",      # path prefix filter
    cpp=True,               # C/C++ files only
    js=True,                # JS/TS files only
    webidl=True,            # WebIDL files only
    c_lang=True,            # C files only
    java=True,              # Java/Kotlin files only
    regexp=False,
    case=False,
    limit=50,
    context=2,              # surrounding lines
)
# → list of (path, line_number, line_content)
```

### get_file

```python
content = client.get_file("dom/media/webaudio/AudioNode.h")
```

### get_definition

```python
source = client.get_definition("AudioNode::Connect", path_filter="dom/media")
```

### search_call_graph

```python
json_str = client.search_call_graph(
    calls_from="mozilla::dom::AudioNode::Connect",
    # calls_to="...",
    # calls_between=("AudioContext", "AudioNode"),
    depth=2,
)
```

### search_field_layout

```python
json_str = client.search_field_layout("mozilla::dom::AudioContext")
```

### get_gc_info

Returns SpiderMonkey GC hazard analysis for C++ functions.

```python
infos = client.get_gc_info("JSContext::newObject")
# → list of (pretty_name, mangled, can_gc: bool, gc_path: str | None)
```

### get_blame_for_lines

```python
blame = client.get_blame_for_lines("dom/media/webaudio/AudioNode.cpp", [10, 20, 30])
# → list of (line_number, short_hash, commit_message, date)
```

### ping

```python
latency_secs = client.ping()
```

## Async example

```python
import asyncio
from searchfox import AsyncSearchfoxClient

async def main():
    client = AsyncSearchfoxClient()
    content = await client.get_file("dom/media/webaudio/AudioNode.h")
    results = await client.search(query="AudioNode", cpp=True, limit=5)

asyncio.run(main())
```

## Repositories

`mozilla-central` (default), `mozilla-beta`, `mozilla-release`,
`mozilla-esr128`, `mozilla-esr140`, `comm-central`
