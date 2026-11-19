# Release a new version: just release 0.19.0
# Crates are published by GitHub Actions using trusted publishing.
# PyPI wheels are published by CI on tag push.
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    if [[ -n "$(git status --porcelain)" ]] || [[ "$(git branch --show-current)" != main ]]; then
        echo "Release requires a clean working tree on main" >&2
        exit 1
    fi
    git fetch origin main --tags
    if [[ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]]; then
        echo "Local main must match origin/main" >&2
        exit 1
    fi
    if git rev-parse -q --verify "refs/tags/v{{version}}" >/dev/null; then
        echo "Tag v{{version}} already exists" >&2
        exit 1
    fi

    # Bump versions
    sed -i.bak "s/^version = \".*\"/version = \"{{version}}\"/" Cargo.toml pyproject.toml
    sed -i.bak "s/searchfox-lib = { version = \"[^\"]*\"/searchfox-lib = { version = \"{{version}}\"/" searchfox-cli/Cargo.toml searchfox-py/Cargo.toml

    rm Cargo.toml.bak pyproject.toml.bak searchfox-cli/Cargo.toml.bak searchfox-py/Cargo.toml.bak

    cargo check
    cargo fmt -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked

    git add Cargo.toml Cargo.lock pyproject.toml searchfox-cli/Cargo.toml searchfox-py/Cargo.toml
    git commit -m "Bump version to {{version}}"
    git push origin main

    # Wait for the CI run for this exact commit before publishing the tag.
    release_commit=$(git rev-parse HEAD)
    run_id=""
    for attempt in {1..60}; do
        run_id=$(gh run list --workflow ci.yml --commit "$release_commit" --event push --json databaseId --jq '.[0].databaseId // empty')
        [[ -n "$run_id" ]] && break
        sleep 5
    done
    if [[ -z "$run_id" ]]; then
        echo "CI run did not appear; release tag was not pushed" >&2
        exit 1
    fi
    gh run watch "$run_id" --exit-status
    git tag -a "v{{version}}" -m "Release v{{version}}"
    git push origin "v{{version}}"

    echo "GitHub Actions will publish the crates, release binaries, and Python wheels."

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
