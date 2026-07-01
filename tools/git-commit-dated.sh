#!/usr/bin/env bash
# git-commit-dated.sh — commit with correct author and committer dates
#
# Usage: git-commit-dated.sh [-m <message>] [-S] [--amend] [extra git-commit args]
#
# Preserves the author date from the parent commit (or last commit when amending)
# and sets the committer date to match, preventing date drift during rebase/squash.

set -euo pipefail

PARENT_DATE=""
AMEND=""
MSG=""
SIGN=""
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m) MSG="$2"; shift 2 ;;
    -S) SIGN="-S"; shift ;;
    --amend) AMEND="--amend"; shift ;;
    --) shift; EXTRA_ARGS+=("$@"); break ;;
    *) EXTRA_ARGS+=("$1"); shift ;;
  esac
done

# Get the date from the parent commit (or last commit if amending)
if [ -n "$AMEND" ]; then
  PARENT_DATE=$(git log -1 --format=%aD HEAD 2>/dev/null || echo "")
else
  PARENT_DATE=$(git log -1 --format=%aD HEAD 2>/dev/null || echo "")
fi

if [ -z "$PARENT_DATE" ]; then
  echo "Warning: could not determine parent commit date, using current time"
  exec git commit $SIGN $AMEND ${MSG:+-m "$MSG"} "${EXTRA_ARGS[@]}"
fi

export GIT_COMMITTER_DATE="$PARENT_DATE"
exec git commit $SIGN $AMEND ${MSG:+-m "$MSG"} --date="$PARENT_DATE" "${EXTRA_ARGS[@]}"
