---
description: Tag a new release
invocation: release
---

# Release Process

1. Bump version in all `Cargo.toml` and `pyproject.toml` files
2. Run `cargo fmt`
3. Run `cargo clippy --all-targets --all-features` (must be clean)
4. Run `cargo build --release` (must succeed)
5. Commit: `git commit -am "Bump version to X.Y.Z"`
6. Tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z\n\n- Feature 1\n- Feature 2"`
7. Push: `git push origin main && git push origin vX.Y.Z`
8. Publish Rust crates: `cargo publish -p searchfox-lib && cargo publish -p searchfox-cli`
9. Publish Python package for all platforms using zig cross-compilation (requires `zig`, `cargo-zigbuild`, and a PyPI API token):
   ```
   mkdir -p /tmp/searchfox-wheels
   for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu; do
     uvx maturin build --manifest-path searchfox-py/Cargo.toml --zig --target $target --release --out /tmp/searchfox-wheels
   done
   uvx twine upload --username __token__ --password <pypi-token> /tmp/searchfox-wheels/*
   ```
   Install prerequisites if missing: `sudo apt install zig && cargo install cargo-zigbuild`
   rustup targets needed: `rustup target add aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin`
