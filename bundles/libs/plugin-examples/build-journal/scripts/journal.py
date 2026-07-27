#!/usr/bin/env python3
"""Append one structured record for an OmniDoc build lifecycle hook."""

from __future__ import annotations

import datetime
import json
import os
import pathlib


def main() -> None:
    root = pathlib.Path(os.environ["OMNIDOC_PROJECT_DIR"]).resolve()
    cache = root / ".omnidoc-cache"
    cache.mkdir(parents=True, exist_ok=True)
    record = {
        "timestamp": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .isoformat(),
        "hook": os.environ.get("OMNIDOC_HOOK", ""),
        "plugin": os.environ.get("OMNIDOC_PLUGIN_KEY", ""),
        "output": os.environ.get("OMNIDOC_OUTPUT", ""),
        "target": os.environ.get("OMNIDOC_TARGET", ""),
        "pid": os.getpid(),
    }
    with (cache / "plugin-build-journal.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
