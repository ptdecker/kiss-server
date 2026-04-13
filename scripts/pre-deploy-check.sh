#!/usr/bin/env bash
set -euo pipefail

# pre-deploy-check.sh
# Validates that everything is consistent before just deploy creates a tag.
# Called automatically by the 'just deploy VERSION' recipe.
#
# Checks:
#   1. Working tree is clean (no uncommitted changes)
#   2. Currently on the main branch
#   3. Cargo.toml version matches VERSION arg
#   4. CHANGELOG.md has a ## [VERSION] entry
#
# Usage: bash scripts/pre-deploy-check.sh VERSION

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
  echo "ERROR: VERSION argument required."
  echo "  Usage: bash scripts/pre-deploy-check.sh VERSION"
  exit 1
fi

FAILED=0

# ─── Check 1: Clean working tree ─────────────────────────────────────────────

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "ERROR: Uncommitted changes present. Commit or stash before deploying."
  FAILED=1
fi

# ─── Check 2: On main branch ─────────────────────────────────────────────────

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
  echo "ERROR: Must be on main to deploy (currently on '$BRANCH')."
  FAILED=1
fi

# ─── Check 3: Cargo.toml version matches ─────────────────────────────────────

CARGO_VERSION=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")
if [ "$CARGO_VERSION" != "$VERSION" ]; then
  echo "ERROR: Cargo.toml version ($CARGO_VERSION) doesn't match deploy version ($VERSION)."
  echo "  Run: just bump $VERSION"
  FAILED=1
fi

# ─── Check 4: CHANGELOG entry exists ─────────────────────────────────────────

if ! grep -q "^## \[$VERSION\]" CHANGELOG.md; then
  echo "ERROR: CHANGELOG.md has no '## [$VERSION]' entry."
  echo "  Add a release section before deploying."
  FAILED=1
fi

# ─── Result ───────────────────────────────────────────────────────────────────

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "Pre-deploy checks passed for v$VERSION."
