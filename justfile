# Release a new version: just release 0.19.0
# Requires crates.io credentials (CARGO_REGISTRY_TOKEN or 'cargo login').
# PyPI wheels are published by CI on tag push.
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    # Fail before the irreversible commit/tag/push if we can't publish afterwards.
    if [ -z "${CARGO_REGISTRY_TOKEN:-}" ] && ! grep -q token "${CARGO_HOME:-$HOME/.cargo}/credentials.toml" 2>/dev/null; then
        echo "error: no crates.io credentials (set CARGO_REGISTRY_TOKEN or run 'cargo login')" >&2
        echo "       refusing to start a release that can't be published" >&2
        exit 1
    fi

    # Bump versions
    sed -i '' "s/^version = \".*\"/version = \"{{version}}\"/" Cargo.toml pyproject.toml
    sed -i '' "s/searchfox-lib = { version = \"[^\"]*\"/searchfox-lib = { version = \"{{version}}\"/" searchfox-cli/Cargo.toml searchfox-py/Cargo.toml

    cargo fmt
    cargo clippy --all-targets --all-features
    cargo build --release

    git add -A
    git commit -m "Bump version to {{version}}"
    git tag -a "v{{version}}" -m "Release v{{version}}"
    git push origin main
    git push origin "v{{version}}"

    cargo publish -p searchfox-lib
    cargo publish -p searchfox-cli

    echo "Python wheels will be built and published to PyPI by CI on tag push."

# Rebuild the Python bindings and install them into the active virtualenv.
# Activate your venv first, or pass VIRTUAL_ENV explicitly:
#   source /path/to/venv/bin/activate && just develop
#   VIRTUAL_ENV=/path/to/venv just develop
develop:
    #!/usr/bin/env bash
    set -e
    cargo build -p searchfox-py --release
    pyver=$($VIRTUAL_ENV/bin/python3 -c 'import sys; v=sys.version_info; print(f"{v.major}{v.minor}")')
    cp target/release/libsearchfox.so python/searchfox/searchfox.abi3.so
    cp target/release/libsearchfox.so "python/searchfox/searchfox.cpython-${pyver}-x86_64-linux-gnu.so"
