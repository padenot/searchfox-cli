# searchfox-cli

[![Crates.io](https://img.shields.io/crates/v/searchfox-cli.svg)](https://crates.io/crates/searchfox-cli)
[![PyPI](https://img.shields.io/pypi/v/searchfox.svg)](https://pypi.org/project/searchfox/)
[![CI](https://github.com/padenot/searchfox-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/padenot/searchfox-cli/actions/workflows/ci.yml)

Search Mozilla source code from the terminal: text, identifiers, definitions,
call graphs, C++ layouts, and SpiderMonkey GC hazards.

| Interface | Package |
| --- | --- |
| Command line | `searchfox-cli` on crates.io |
| Rust library | `searchfox-lib` on crates.io |
| Python bindings | `searchfox` on PyPI; see [Python API](python/README.md) |

## Install

```sh
cargo install searchfox-cli
```

Prebuilt binaries are available from [GitHub Releases](https://github.com/padenot/searchfox-cli/releases).
For Python, use `pip install searchfox`.

## Search

```sh
searchfox-cli -q AudioStream
searchfox-cli -q '^Audio.*' -r -C -l 10
searchfox-cli -q AudioStream -p '^dom/media' --cpp
searchfox-cli -p PContent.ipdl
searchfox-cli --id AudioContext
searchfox-cli --define 'AudioContext::CreateGain'
searchfox-cli --get-file dom/media/AudioStream.h --lines 100-150
searchfox-cli -q 'text:function main()' --context 3
```

`--id` accepts a source identifier. `--symbol` expects a mangled symbol from
Searchfox output, or an unmangled C function name; use `--id` for plain C++ names.
`--context` adds surrounding lines to `text:` and `re:` queries.

The default repository is `mozilla-central`. Use `-R` for another repository
served by Searchfox, such as `mozilla-beta`, `mozilla-release`, or `comm-central`.
Set `SEARCHFOX_BASE_URL` to use another server:

```sh
SEARCHFOX_BASE_URL=http://127.0.0.1:16995 searchfox-cli -q AudioStream
```

## Inspect code

```sh
searchfox-cli --calls-from 'mozilla::dom::AudioContext::CreateGain' --depth 2
searchfox-cli --calls-to 'mozilla::AudioCallbackDriver::Start'
searchfox-cli --calls-between 'AudioContext,AudioNode' --depth 2
searchfox-cli --field-layout 'mozilla::dom::AudioContext'
searchfox-cli --can-gc 'JSContext::newObject'
searchfox-cli --function-at dom/media/AudioStream.cpp:42
searchfox-cli --spec-refs 'https://html.spec.whatwg.org/#navigate'
```

| Option | Purpose |
| --- | --- |
| `--cpp`, `--c`, `--js`, `--webidl`, `--java` / `--kt` | Filter by language |
| `--exclude-tests`, `--exclude-generated` | Omit those file categories |
| `--only-tests`, `--only-generated`, `--only-normal` | Restrict to one file category |
| `--blame` | Include commit information |
| `--link`, `--permalink` | Return result links instead of source text |
| `--log-requests` | Show HTTP timing and response sizes |

Run `searchfox-cli --help` for the full option list.

## File cache

File fetches use a SQLite cache at `$XDG_CACHE_HOME/searchfox-cli/cache.db`, or
`~/.cache/searchfox-cli/cache.db`. Contents are fresh for one hour; older entries
are revalidated with `ETag` / `Last-Modified` when available. Entries older than
seven days are pruned when the cache opens.

```sh
searchfox-cli --get-file dom/media/AudioStream.h --force-refetch
searchfox-cli --get-file dom/media/AudioStream.h --no-cache
searchfox-cli --clear-cache
```

## Python

```python
from searchfox import SearchfoxClient

client = SearchfoxClient("mozilla-central")
results = client.search(query="AudioStream", limit=10)
source = client.get_definition("AudioContext::CreateGain")
```

`AsyncSearchfoxClient` exposes the same operations as awaitables. See the
[API reference](python/README.md) and [examples](python/examples/README.md).

## Development

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

To build the Python extension, activate a virtualenv, install `maturin`, and run
`maturin develop --manifest-path searchfox-py/Cargo.toml` from the repository root.

## Publishing

**Pushing a `vVERSION` tag publishes the Rust crates, binaries, and Python wheels.**
The tagged commit must contain matching package versions. GitHub write access
is enough for routine releases; no local publishing token is needed.

See [Publishing](docs/PUBLISHING.md) for version files, checks, tag commands,
release verification, retries, and ownership. `just release VERSION` is an
optional shortcut, not a requirement.

## License

[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
