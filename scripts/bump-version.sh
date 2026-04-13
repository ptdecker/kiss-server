#!/usr/bin/env bash
set -euo pipefail

# bump-version.sh
# Updates Cargo.toml to the given version and regenerates Cargo.lock.
# Run this on a feature branch before updating CHANGELOG.md and opening a PR.
#
# Usage: bash scripts/bump-version.sh VERSION
# Example: bash scripts/bump-version.sh 1.3.0

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
  echo "ERROR: VERSION argument required."
  echo "  Usage: bash scripts/bump-version.sh VERSION"
  exit 1
fi

if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "ERROR: VERSION must be semver (X.Y.Z). Got: $VERSION"
  exit 1
fi

# ─── Step 1: Update Cargo.toml ────────────────────────────────────────────────

echo "==> Step 1: Update Cargo.toml to $VERSION"
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
echo "  Done."

# ─── Step 2: Regenerate Cargo.lock ───────────────────────────────────────────

echo "==> Step 2: Regenerate Cargo.lock"
cargo update --workspace
echo "  Done."

# ─── Checklist ────────────────────────────────────────────────────────────────

echo ""
echo "=== Cargo.toml and Cargo.lock updated to $VERSION ==="
echo ""
echo "Next steps:"
echo "  1. Add a '## [$VERSION] - $(date +%Y-%m-%d)' section to CHANGELOG.md"
echo "  2. git add Cargo.toml Cargo.lock CHANGELOG.md"
echo "  3. git commit -m \"chore: release v$VERSION\""
echo "  4. git push && gh pr create"
echo "  5. After merge: just deploy $VERSION"
