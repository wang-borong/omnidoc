# Documentation quality gate

This example adds a `lint_rule` hook. `omnidoc lint` and strict builds scan
Markdown sources while ignoring generated/build directories and fenced code.

The rule reports:

- `TODO` and `FIXME` markers as warnings;
- Markdown files that contain prose but no level-one heading as warnings.

The script only reads project sources and always emits OmniDoc's portable
`severity:path:line:column:message` diagnostic format. Edit `scripts/lint.py`
to add organization-specific wording, front-matter, or heading rules.
