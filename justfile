# Rebuild the Python bindings and install them into the active virtualenv.
# Activate your venv first, or pass VIRTUAL_ENV explicitly:
#   source /path/to/venv/bin/activate && just develop
#   VIRTUAL_ENV=/path/to/venv just develop
develop:
    $VIRTUAL_ENV/bin/maturin develop
