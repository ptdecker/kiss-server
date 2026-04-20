#!/usr/bin/env bash
# install-hooks.sh — install git hooks from scripts/
# Usage: bash scripts/install-hooks.sh  OR  just install-hooks

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

cp "$REPO_ROOT/scripts/pre-commit" "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-commit"

echo "Installed: .git/hooks/pre-commit"
