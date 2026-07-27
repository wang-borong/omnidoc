# Build lifecycle journal

This example uses both `pre_build` and `post_build`. It appends compact JSON
records to `.omnidoc-cache/plugin-build-journal.jsonl`, including the hook,
output format, target name, process ID, and UTC timestamp.

The journal is useful when diagnosing multi-output builds or as a starting
point for notifications and artifact publishing. It stays under OmniDoc's cache
directory, so it does not become a document dependency or trigger rebuilds.
