---
name: release
description: Tag a new version, make a new release
---

# Release Process

Run:

```
CARGO_REGISTRY_TOKEN=<token> just release X.Y.Z
```

CI will build and publish Python wheels for all platforms on tag push.
Requires a `PYPI_TOKEN` secret and a `pypi` environment configured in the GitHub repo settings.
