#!/usr/bin/env bash
# Changelog ledger guard (see AGENTS.md, Commit And Integration Governance > Changelog).
#
# CHANGELOG.md is a strict release ledger: it carries no Unreleased section, and every
# `## [X.Y.Z]` version heading has a matching `[X.Y.Z]: https://github.com/<owner>/<repo>/releases/tag/vX.Y.Z`
# footer link, so a reader can always click through from a version to its release.
set -euo pipefail
cd "$(dirname "$0")/.."

file="CHANGELOG.md"
if [ ! -f "$file" ]; then
  echo "changelog-guard: CHANGELOG.md is missing" >&2
  exit 1
fi

if grep -qiE '^## \[?unreleased\]?' "$file"; then
  echo "changelog-guard: CHANGELOG.md is a release ledger and must not carry an Unreleased section" >&2
  exit 1
fi

headings=$(grep -oE '^## \[[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?\]' "$file" | sed -E 's/^## \[(.*)\]$/\1/' || true)

if [ -z "$headings" ]; then
  echo "changelog-guard: clean — no version headings yet"
  exit 0
fi

missing=0
while IFS= read -r version; do
  escaped=${version//./\\.}
  if ! grep -qE "^\[${escaped}\]: https://github\.com/[^/ ]+/[^/ ]+/releases/tag/v${escaped}$" "$file"; then
    echo "changelog-guard: version heading [$version] has no footer link to its releases/tag/v$version" >&2
    missing=1
  fi
done <<< "$headings"

if [ "$missing" -ne 0 ]; then
  echo "changelog-guard: FAILED — add a [X.Y.Z]: .../releases/tag/vX.Y.Z footer line for every version heading" >&2
  exit 1
fi

echo "changelog-guard: clean — every version heading has a matching footer link"
