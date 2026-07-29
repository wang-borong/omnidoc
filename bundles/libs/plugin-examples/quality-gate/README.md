# Quality gate plugin

This package is a Pandoc Lua filter. It records warnings for TODO/FIXME
markers and for documents without an H1. Set this metadata value to make the
filter fail the build when it finds an issue:

```yaml
quality-gate-fail: true
```

Install, trust, and enable it explicitly:

```bash
omnidoc plugin install-example quality-gate --project ./docs
omnidoc plugin trust omnidoc/quality-gate@=1.0.0 --project ./docs
omnidoc plugin enable omnidoc/quality-gate@=1.0.0 ./docs
```
