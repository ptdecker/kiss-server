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

if [ -n "$(git status --porcelain)" ]; then
  echo "ERROR: Working tree is not clean. Commit, stash, or remove changes before deploying."
  FAILED=1
fi

# ─── Check 2: On main branch ─────────────────────────────────────────────────

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
  echo "ERROR: Must be on main to deploy (currently on '$BRANCH')."
  FAILED=1
fi

# ─── Check 3: Cargo.toml version matches ─────────────────────────────────────

CARGO_VERSION=$(awk -F'"' '/^\[package\]/{p=1} p && /^version/{print $2; exit}' Cargo.toml)
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
