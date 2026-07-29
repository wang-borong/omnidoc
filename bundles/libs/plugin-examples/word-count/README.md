# Word count command plugin

This package declares no automatic filter. Its command runs only when invoked
explicitly through `pandoc lua` by OmniDoc:

```bash
omnidoc plugin install-example word-count --project ./docs
omnidoc plugin trust omnidoc/word-count@=1.0.0 --project ./docs
omnidoc plugin run omnidoc/word-count word-count --project ./docs -- main.md chapters/intro.md
```

Each output line contains the input path, whitespace-delimited word count, and
UTF-8 character count separated by tabs.
