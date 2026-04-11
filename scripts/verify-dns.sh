#!/usr/bin/env bash
set -euo pipefail

# verify-dns.sh
# Smoke test for DNS configuration of ptodd.org.
# Checks CloudFront-routed DNS: CNAME resolution, HTTPS content,
# CloudFront response headers, and apex redirect chain.
#
# Usage: bash scripts/verify-dns.sh
# Requirements: dig, curl (both present on macOS and Amazon Linux 2023)
# Run from: Developer machine (not EC2)

ELASTIC_IP="54.83.192.65"
DOMAIN="ptodd.org"
WWW_DOMAIN="www.ptodd.org"
CLOUDFRONT_DOMAIN="d3ahc2eiiqz0iu.cloudfront.net"
FAILURES=0

# ─── Step 1: www CNAME check (DNS-04) ────────────────────────────────────────
# Verify www.ptodd.org CNAME points to cloudfront.net.

echo "==> Step 1: www CNAME check for $WWW_DOMAIN"

CNAME=$(dig +short "$WWW_DOMAIN" CNAME)

if echo "$CNAME" | grep -q "cloudfront.net"; then
  echo "  PASS: $WWW_DOMAIN CNAME points to cloudfront.net ($CNAME)"
else
  echo "  FAIL: $WWW_DOMAIN CNAME is '$CNAME' (expected cloudfront.net)"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 2: www does not resolve to EC2 IP (DNS-04) ─────────────────────────
# Verify www.ptodd.org A record resolution does NOT return the EC2 Elastic IP.

echo "==> Step 2: www does not resolve to EC2 IP"

WWW_RESOLVED_IP=$(dig +short "$WWW_DOMAIN" A | tail -1)

if [ "$WWW_RESOLVED_IP" != "$ELASTIC_IP" ]; then
  echo "  PASS: $WWW_DOMAIN does not resolve to EC2 IP (got $WWW_RESOLVED_IP)"
else
  echo "  FAIL: $WWW_DOMAIN still resolves to EC2 IP $ELASTIC_IP"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 3: HTTPS 200 + CloudFront headers for www (DNS-04) ─────────────────
# Verify https://www.ptodd.org/ returns HTTP 200 with x-cache header.

echo "==> Step 3: HTTPS 200 + CloudFront headers for $WWW_DOMAIN"

HTTP_CODE=$(curl -so /dev/null -w "%{http_code}" --max-time 15 "https://$WWW_DOMAIN/")
HEADERS=$(curl -sI --max-time 15 "https://$WWW_DOMAIN/")

if [ "$HTTP_CODE" = "200" ] && echo "$HEADERS" | grep -qi "x-cache"; then
  echo "  PASS: https://$WWW_DOMAIN/ returns $HTTP_CODE with CloudFront headers"
else
  echo "  FAIL: https://$WWW_DOMAIN/ returned $HTTP_CODE (expected 200 with x-cache header)"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 4: HTTPS content check for www (DNS-04) ────────────────────────────
# Verify https://www.ptodd.org/ returns "Hello World" content.

echo "==> Step 4: HTTPS content for $WWW_DOMAIN"

if curl -fsL --max-time 15 "https://$WWW_DOMAIN/" | grep -q "Hello World"; then
  echo "  PASS: https://$WWW_DOMAIN/ returns Hello World"
else
  echo "  FAIL: https://$WWW_DOMAIN/ did not return Hello World"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 5: Apex redirect chain (DNS-05) ────────────────────────────────────
# Verify http://ptodd.org 301-redirects to https://www.ptodd.org/ and final
# response is 200.

echo "==> Step 5: Apex redirect chain for $DOMAIN"

RESULT=$(curl -so /dev/null -w "%{http_code} %{url_effective}" -L --max-time 15 "http://$DOMAIN/")
APEX_HTTP_CODE=$(echo "$RESULT" | awk '{print $1}')
APEX_FINAL_URL=$(echo "$RESULT" | awk '{print $2}')

if [ "$APEX_HTTP_CODE" = "200" ] && echo "$APEX_FINAL_URL" | grep -q "https://www.ptodd.org"; then
  echo "  PASS: http://$DOMAIN/ redirects to $APEX_FINAL_URL ($APEX_HTTP_CODE)"
else
  echo "  FAIL: http://$DOMAIN/ -> $APEX_FINAL_URL ($APEX_HTTP_CODE) (expected 200 at https://www.ptodd.org/)"
  FAILURES=$((FAILURES + 1))
fi

# ─── Step 6: Apex no longer resolves to EC2 IP (DNS-05) ──────────────────────
# Verify ptodd.org A record no longer points to the EC2 Elastic IP.
# GoDaddy domain forwarding replaces the A record with their forwarding servers.

echo "==> Step 6: Apex no longer resolves to EC2 IP"

APEX_IP=$(dig +short "$DOMAIN" A | tail -1)

if [ "$APEX_IP" != "$ELASTIC_IP" ]; then
  echo "  PASS: $DOMAIN no longer resolves to EC2 IP (got $APEX_IP)"
else
  echo "  FAIL: $DOMAIN still resolves to EC2 IP $ELASTIC_IP"
  FAILURES=$((FAILURES + 1))
fi

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""

if [ "$FAILURES" -eq 0 ]; then
  echo "All checks passed. DNS configuration is correct."
  exit 0
else
  echo "$FAILURES check(s) failed."
  echo "If DNS was just changed, wait up to 15 minutes for propagation and re-run."
  exit 1
fi
