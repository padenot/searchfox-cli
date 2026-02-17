---
name: release
description: Tag a new version, make a new release
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
9. Publish Python package: `cd searchfox-py && uvx maturin publish`
