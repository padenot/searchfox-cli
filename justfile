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
