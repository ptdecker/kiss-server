#!/usr/bin/env bash
set -euo pipefail

# setup-webroot.sh
# Idempotent script to create the web root directory and write index.html
# for the kiss-server static file server on an Amazon Linux 2023 EC2 instance.
#
# Usage: bash scripts/setup-webroot.sh
# Requirements: Must be run on the EC2 instance (not the local machine)
#
# Steps:
#   1. Create /var/www/ptodd.org/ directory
#   2. Set ownership (root:root, world-readable)
#   3. Write index.html placeholder

WEBROOT="/var/www/ptodd.org"
INDEX_FILE="$WEBROOT/index.html"

# ─── Step 1: Create webroot directory ─────────────────────────────────────────

echo "==> Step 1: Webroot directory"

if [ -d "$WEBROOT" ]; then
  echo "  Webroot $WEBROOT already exists, skipping."
else
  echo "  Creating webroot $WEBROOT..."
  sudo mkdir -p "$WEBROOT"
fi

# ─── Step 2: Set ownership ────────────────────────────────────────────────────

echo "==> Step 2: Permissions"
sudo chown root:root "$WEBROOT"
sudo chmod 755 "$WEBROOT"
echo "  Permissions set: root:root 755"

# ─── Step 3: Write index.html ─────────────────────────────────────────────────

echo "==> Step 3: index.html"

if [ -f "$INDEX_FILE" ]; then
  echo "  $INDEX_FILE already exists, skipping."
else
  echo "  Writing $INDEX_FILE..."
  sudo tee "$INDEX_FILE" > /dev/null << 'HTML'
<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>ptodd.org</title></head>
<body><h1>Hello World</h1></body>
</html>
HTML
  sudo chmod 644 "$INDEX_FILE"
  echo "  $INDEX_FILE written."
fi

echo ""
echo "setup-webroot.sh complete."
