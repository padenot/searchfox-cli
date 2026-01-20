#!/bin/bash
# Install git pre-commit hook

HOOK_DIR=".git/hooks"
HOOK_FILE="$HOOK_DIR/pre-commit"

cat > "$HOOK_FILE" << 'EOF'
#!/bin/bash
set -e

echo "Running cargo fmt --check..."
cargo fmt -- --check

echo "Running cargo clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "All checks passed."
EOF

chmod +x "$HOOK_FILE"
echo "Pre-commit hook installed successfully."
