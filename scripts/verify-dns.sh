#!/usr/bin/env bash
set -euo pipefail

# verify-dns.sh
# Smoke test for DNS configuration of ptodd.org.
# Checks A record, CNAME resolution, and HTTP content for both
# ptodd.org and www.ptodd.org.
#
# Usage: bash scripts/verify-dns.sh
# Requirements: dig, curl (both present on macOS and Amazon Linux 2023)
# Run from: Developer machine (not EC2)

ELASTIC_IP="54.83.192.65"
DOMAIN="ptodd.org"
WWW_DOMAIN="www.ptodd.org"
FAILURES=0

# ─── Step 1: A record check (DNS-01) ─────────────────────────────────────────
# Verify ptodd.org A record resolves to the Elastic IP.

echo "==> Step 1: A record for $DOMAIN"

RESOLVED_IP=$(dig +short "$DOMAIN" A)

if [ "$RESOLVED_IP" = "$ELASTIC_IP" ]; then
  echo "  PASS: $DOMAIN resolves to $ELASTIC_IP"
else
  echo "  FAIL: $DOMAIN resolves to '$RESOLVED_IP' (expected '$ELASTIC_IP')"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 2: www resolution check (DNS-02) ───────────────────────────────────
# Verify www.ptodd.org CNAME chain resolves to the Elastic IP.
# dig +short follows CNAME chains and returns the final A record IP.

echo "==> Step 2: www resolution for $WWW_DOMAIN"

WWW_RESOLVED_IP=$(dig +short "$WWW_DOMAIN" A | tail -1)

if [ "$WWW_RESOLVED_IP" = "$ELASTIC_IP" ]; then
  echo "  PASS: $WWW_DOMAIN resolves to $ELASTIC_IP"
else
  echo "  FAIL: $WWW_DOMAIN resolves to '$WWW_RESOLVED_IP' (expected '$ELASTIC_IP')"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 3: HTTP content check for ptodd.org (DNS-03 part 1) ────────────────
# Verify HTTP response from ptodd.org contains "Hello World".

echo "==> Step 3: HTTP content for $DOMAIN"

if curl -fs --max-time 10 "http://$DOMAIN/" | grep -q "Hello World"; then
  echo "  PASS: http://$DOMAIN/ returns Hello World"
else
  echo "  FAIL: http://$DOMAIN/ did not return Hello World"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 4: HTTP content check for www.ptodd.org (DNS-03 part 2) ────────────
# Verify HTTP response from www.ptodd.org contains "Hello World".

echo "==> Step 4: HTTP content for $WWW_DOMAIN"

if curl -fs --max-time 10 "http://$WWW_DOMAIN/" | grep -q "Hello World"; then
  echo "  PASS: http://$WWW_DOMAIN/ returns Hello World"
else
  echo "  FAIL: http://$WWW_DOMAIN/ did not return Hello World"
  FAILURES=$((FAILURES + 1))
fi

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""

if [ "$FAILURES" -eq 0 ]; then
  echo "All checks passed. DNS configuration is correct."
  exit 0
else
  echo "$FAILURES check(s) failed."
  echo "If DNS was just changed, wait up to 1 hour for propagation and re-run."
  exit 1
fi
