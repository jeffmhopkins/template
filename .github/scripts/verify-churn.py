"""Sections re-verified far more often than their text is edited, from the ledger's history.

Reads `git log -p -U1 --format='@@COMMIT %H' -- docs/verified.toml` on stdin and prints a
Markdown list. A verification is a commit that changes any stamp of the section; an edit is
one that changes the hash of its `body` input. Best-effort: a section renamed or moved keeps
no history across the move.

Usage: git log … | python3 -I verify-churn.py [--min-verifies N] [--ratio R]
"""

import argparse
import re
import sys
from collections import defaultdict

KEY = re.compile(r'^[ +-]\["([^"]+)"\."([^"]+)"\]\s*$')
STAMP = re.compile(r'^([+-])stamp\s*=\s*\{\s*hash\s*=\s*"([^"]*)"')


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--min-verifies", type=int, default=5)
    ap.add_argument("--ratio", type=float, default=4.0)
    args = ap.parse_args()

    verifies: dict[str, int] = defaultdict(int)
    edits: dict[str, int] = defaultdict(int)

    def flush(changes: dict[tuple[str, str], dict[str, str]]) -> None:
        sections = set()
        for (section, inp), sides in changes.items():
            sections.add(section)
            if inp == "body" and sides.get("-") != sides.get("+"):
                edits[section] += 1
        for section in sections:
            verifies[section] += 1

    changes: dict[tuple[str, str], dict[str, str]] = {}
    key = None
    for line in sys.stdin:
        line = line.rstrip("\n")
        if line.startswith("@@COMMIT "):
            flush(changes)
            changes, key = {}, None
            continue
        if line.startswith("@@"):
            key = None
            continue
        m = KEY.match(line)
        if m:
            key = (m.group(1), m.group(2))
            continue
        m = STAMP.match(line)
        if m and key is not None:
            changes.setdefault(key, {})[m.group(1)] = m.group(2)
    flush(changes)

    flagged = [
        (s, v, edits[s])
        for s, v in verifies.items()
        if v >= args.min_verifies and v >= args.ratio * max(edits[s], 1)
    ]
    if not flagged:
        print("None.")
        return 0
    print("Re-verified at least "
          f"{args.ratio:g}× as often as edited (and {args.min_verifies}+ times). "
          "Check that each re-verification read the section, and whether its `covers` is too wide.")
    print()
    for s, v, e in sorted(flagged, key=lambda x: (-x[1], x[0])):
        print(f"- `{s}`: {v} verifications, {e} edits")
    return 0


if __name__ == "__main__":
    sys.exit(main())
