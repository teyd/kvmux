#!/usr/bin/env bash
# Every commit in RANGE must be a Conventional Commit: type(scope)!: subject
set -euo pipefail

range="${1:?usage: check-commits.sh <git revision range>}"
pattern='^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9._-]+\))?!?: .{1,72}$'
status=0

while IFS= read -r line; do
  sha="${line%% *}"
  subject="${line#* }"
  if ! [[ "$subject" =~ $pattern ]]; then
    echo "not a conventional commit: $sha $subject" >&2
    status=1
  fi
done < <(git log --no-merges --format='%h %s' "$range")

exit $status
