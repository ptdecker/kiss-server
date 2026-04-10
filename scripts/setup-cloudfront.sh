#!/usr/bin/env bash
set -euo pipefail

# setup-cloudfront.sh
# Idempotent script to provision a CloudFront distribution for kiss-server.
# Creates a distribution with EC2:80 as HTTP-only origin, HTTPS termination
# via ACM certificate, HTTP-to-HTTPS redirect, Host header forwarding,
# TLSv1.2_2021 minimum, gzip/brotli compression, and TTL=0 (no caching).
# Polls until Status is "Deployed", then writes docs/aws-resources.md.
#
# Usage: bash scripts/setup-cloudfront.sh
# Requirements: AWS CLI v2 configured with the 'kiss' profile
#
# Resources provisioned:
#   - CloudFront distribution: kiss-server-www
#     - Origin: 54.83.192.65:80 (HTTP-only, EC2 Elastic IP)
#     - Alternate domain: www.ptodd.org
#     - ACM certificate: us-east-1 (TLS termination at CloudFront)
#     - Cache policy: Managed-CachingDisabled (TTL=0)
#     - Origin request policy: Managed-HostHeaderOnly (forwards Host header)
#     - Viewer protocol: redirect-to-https
#     - Minimum TLS: TLSv1.2_2021
#     - Compression: enabled (gzip/brotli)
#   - docs/aws-resources.md (written with all project AWS resource IDs)
#
# NOTE: Do NOT pass a region flag to any aws cloudfront command.
# CloudFront is a global service; a region flag causes endpoint errors.

PROFILE="kiss"
CERT_ARN="arn:aws:acm:us-east-1:859953692821:certificate/5df0a174-daab-4126-9f12-67ac0d51a760"
ORIGIN_DOMAIN="54.83.192.65"
COMMENT="kiss-server-www"

# ─── Step 1: Idempotency check ────────────────────────────────────────────────

echo "==> Step 1: Check for existing distribution"

EXISTING_ID=$(aws cloudfront list-distributions \
  --profile "$PROFILE" \
  --query "DistributionList.Items[?Comment=='kiss-server-www'].Id | [0]" \
  --output text 2>/dev/null || echo "")

if [ -z "$EXISTING_ID" ] || [ "$EXISTING_ID" = "None" ]; then

  # ─── Step 2: Create distribution ──────────────────────────────────────────

  echo "==> Step 2: Create CloudFront distribution"
  echo "  Comment:        $COMMENT"
  echo "  Origin:         $ORIGIN_DOMAIN:80 (HTTP-only)"
  echo "  Alternate DNS:  www.ptodd.org"
  echo "  Cache policy:   Managed-CachingDisabled (TTL=0)"
  echo "  TLS minimum:    TLSv1.2_2021"

  DISTRIBUTION_ID=$(aws cloudfront create-distribution \
    --profile "$PROFILE" \
    --query 'Distribution.Id' \
    --output text \
    --distribution-config '{
      "CallerReference": "kiss-server-www-v1",
      "Comment": "kiss-server-www",
      "Aliases": {
        "Quantity": 1,
        "Items": ["www.ptodd.org"]
      },
      "Origins": {
        "Quantity": 1,
        "Items": [{
          "Id": "kiss-server-ec2",
          "DomainName": "'"$ORIGIN_DOMAIN"'",
          "CustomHeaders": {"Quantity": 0},
          "CustomOriginConfig": {
            "HTTPPort": 80,
            "HTTPSPort": 443,
            "OriginProtocolPolicy": "http-only",
            "OriginSSLProtocols": {"Quantity": 0, "Items": []},
            "OriginReadTimeout": 30,
            "OriginKeepaliveTimeout": 5
          }
        }]
      },
      "DefaultCacheBehavior": {
        "TargetOriginId": "kiss-server-ec2",
        "ViewerProtocolPolicy": "redirect-to-https",
        "CachePolicyId": "4135ea2d-6df8-44a3-9df3-4b5a84be39ad",
        "OriginRequestPolicyId": "bf0718e1-ba1e-49d1-88b1-f726733018ae",
        "Compress": true,
        "AllowedMethods": {
          "Quantity": 2,
          "Items": ["GET", "HEAD"],
          "CachedMethods": {"Quantity": 2, "Items": ["GET", "HEAD"]}
        }
      },
      "CacheBehaviors": {"Quantity": 0},
      "CustomErrorResponses": {"Quantity": 0},
      "PriceClass": "PriceClass_100",
      "Enabled": true,
      "HttpVersion": "http2",
      "IsIPV6Enabled": true,
      "ViewerCertificate": {
        "ACMCertificateArn": "'"$CERT_ARN"'",
        "SSLSupportMethod": "sni-only",
        "MinimumProtocolVersion": "TLSv1.2_2021",
        "CloudFrontDefaultCertificate": false
      },
      "Restrictions": {
        "GeoRestriction": {"RestrictionType": "none", "Quantity": 0}
      }
    }')

  echo "  Distribution created: $DISTRIBUTION_ID"

  # ─── Step 3: Wait for Deployed status ─────────────────────────────────────

  echo "==> Step 3: Wait for distribution to deploy"
  echo "  Waiting for distribution to deploy (this may take 10-15 minutes)..."
  echo "  (aws cloudfront wait polls every 60s, max 35 minutes)"
  aws cloudfront wait distribution-deployed \
    --id "$DISTRIBUTION_ID" \
    --profile "$PROFILE"
  echo "  Distribution is Deployed."

else
  DISTRIBUTION_ID="$EXISTING_ID"
  echo "  Distribution already exists: $EXISTING_ID, skipping."
fi

# ─── Step 4: Retrieve domain and ARN ──────────────────────────────────────────

echo "==> Step 4: Retrieve distribution metadata"

DISTRIBUTION_DOMAIN=$(aws cloudfront get-distribution \
  --id "$DISTRIBUTION_ID" \
  --profile "$PROFILE" \
  --query 'Distribution.DomainName' \
  --output text)

DISTRIBUTION_ARN=$(aws cloudfront get-distribution \
  --id "$DISTRIBUTION_ID" \
  --profile "$PROFILE" \
  --query 'Distribution.ARN' \
  --output text)

echo "  Distribution Domain: $DISTRIBUTION_DOMAIN"
echo "  Distribution ARN:    $DISTRIBUTION_ARN"

# ─── Step 5: Write docs/aws-resources.md ──────────────────────────────────────

echo "==> Step 5: Write docs/aws-resources.md"

cat > docs/aws-resources.md <<EOF
# AWS Resources -- kiss-server

| Resource | Value |
|----------|-------|
| EC2 Instance ID | i-0394a6d927c0d9b33 |
| EC2 Elastic IP | 54.83.192.65 |
| ACM Certificate ARN | arn:aws:acm:us-east-1:859953692821:certificate/5df0a174-daab-4126-9f12-67ac0d51a760 |
| CloudFront Distribution ID | $DISTRIBUTION_ID |
| CloudFront Distribution Domain | $DISTRIBUTION_DOMAIN |
| CloudFront Distribution ARN | $DISTRIBUTION_ARN |
EOF

echo "  docs/aws-resources.md written."

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "==> CloudFront distribution ready"
echo "  Distribution ID:     $DISTRIBUTION_ID"
echo "  Distribution Domain: $DISTRIBUTION_DOMAIN"
echo "  Distribution ARN:    $DISTRIBUTION_ARN"
echo "  Alternate Domain:    www.ptodd.org"
echo "  Status:              Deployed"
echo ""
echo "Distribution ID: $DISTRIBUTION_ID"
echo "Distribution Domain: $DISTRIBUTION_DOMAIN"
