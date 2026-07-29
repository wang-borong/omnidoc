# Metadata stamp plugin

This package demonstrates a small output-neutral Pandoc AST transform. It adds
`omnidoc-plugin = "metadata-stamp/1.0.0"` and supplies a generator value only
when the document does not already define one.

```bash
omnidoc plugin install-example metadata-stamp --project ./docs
omnidoc plugin trust omnidoc/metadata-stamp@=1.0.0 --project ./docs
omnidoc plugin enable omnidoc/metadata-stamp@=1.0.0 ./docs
```
