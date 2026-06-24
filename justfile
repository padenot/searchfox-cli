# Release a new version: just release 0.19.0
# Requires CARGO_REGISTRY_TOKEN and PYPI_TOKEN env vars.
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    # Bump versions
    sed -i '' "s/^version = \".*\"/version = \"{{version}}\"/" Cargo.toml pyproject.toml
    sed -i '' "s/searchfox-lib = { version = \".*\"/searchfox-lib = { version = \"{{version}}\"/" searchfox-cli/Cargo.toml searchfox-py/Cargo.toml

    cargo fmt
    cargo clippy --all-targets --all-features
    cargo build --release

    git add -A
    git commit -m "Bump version to {{version}}"
    git tag -a "v{{version}}" -m "Release v{{version}}"
    git push origin main
    git push origin "v{{version}}"

    CARGO_REGISTRY_TOKEN="${CARGO_REGISTRY_TOKEN}" cargo publish -p searchfox-lib
    CARGO_REGISTRY_TOKEN="${CARGO_REGISTRY_TOKEN}" cargo publish -p searchfox-cli

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
