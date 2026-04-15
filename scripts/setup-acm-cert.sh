#!/usr/bin/env bash
# setup-acm-cert.sh
#
# Idempotent script to request an ACM certificate for ptodd.org and www.ptodd.org in us-east-1 with
# DNS validation, then print human-readable CNAME validation instructions formatted for GoDaddy's
# subdomain-prefix-only input requirement.
#
# Usage: bash scripts/setup-acm-cert.sh
# Requirements: AWS CLI v2 configured with the 'kiss' profile
#
# Resources provisioned:
#   - ACM certificate (us-east-1): ptodd.org + www.ptodd.org (DNS-validated, auto-renewing)

set -euo pipefail

REGION="us-east-1"
PROFILE="kiss"
DOMAIN="ptodd.org"
SAN="www.ptodd.org"

# ─── Step 1: Certificate request (idempotent) ─────────────────────────────────

echo "==> Step 1: ACM certificate"

EXISTING=$(aws acm list-certificates \
  --region "$REGION" \
  --profile "$PROFILE" \
  --query "CertificateSummaryList[?DomainName=='ptodd.org'].CertificateArn | [0]" \
  --output text 2>/dev/null || echo "")

if [ -n "$EXISTING" ] && [ "$EXISTING" != "None" ]; then
  CERT_ARN="$EXISTING"
  echo "  Certificate for ptodd.org already exists: $CERT_ARN, skipping request."
else
  echo "  Requesting new certificate for $DOMAIN + $SAN..."
  CERT_ARN=$(aws acm request-certificate \
    --domain-name "$DOMAIN" \
    --subject-alternative-names "$SAN" \
    --validation-method DNS \
    --idempotency-token "ptoddacmcert" \
    --profile "$PROFILE" \
    --region "$REGION" \
    --query 'CertificateArn' \
    --output text)
  echo "  Certificate requested: $CERT_ARN"
fi

# ─── Step 2: Retrieve validation records ──────────────────────────────────────

echo "==> Step 2: Retrieve validation records"
echo "  Waiting for certificate metadata to become available..."
sleep 10

NAME_PTODD=$(aws acm describe-certificate \
  --certificate-arn "$CERT_ARN" \
  --region "$REGION" \
  --profile "$PROFILE" \
  --query "Certificate.DomainValidationOptions[?DomainName=='ptodd.org'].ResourceRecord.Name | [0]" \
  --output text)

VALUE_PTODD=$(aws acm describe-certificate \
  --certificate-arn "$CERT_ARN" \
  --region "$REGION" \
  --profile "$PROFILE" \
  --query "Certificate.DomainValidationOptions[?DomainName=='ptodd.org'].ResourceRecord.Value | [0]" \
  --output text)

NAME_WWW=$(aws acm describe-certificate \
  --certificate-arn "$CERT_ARN" \
  --region "$REGION" \
  --profile "$PROFILE" \
  --query "Certificate.DomainValidationOptions[?DomainName=='www.ptodd.org'].ResourceRecord.Name | [0]" \
  --output text)

VALUE_WWW=$(aws acm describe-certificate \
  --certificate-arn "$CERT_ARN" \
  --region "$REGION" \
  --profile "$PROFILE" \
  --query "Certificate.DomainValidationOptions[?DomainName=='www.ptodd.org'].ResourceRecord.Value | [0]" \
  --output text)

# Strip root domain suffix — GoDaddy requires the subdomain prefix relative to the zone root.
# For ptodd.org records: _abc123.ptodd.org. → _abc123
# For www.ptodd.org records: _abc123.www.ptodd.org. → _abc123.www
PREFIX_PTODD=$(echo "$NAME_PTODD" | sed "s/\\.${DOMAIN}\\.*$//")
PREFIX_WWW=$(echo "$NAME_WWW" | sed "s/\\.${DOMAIN}\\.*$//")

# ─── Step 3: Validation instructions ──────────────────────────────────────────

echo "==> Step 3: Validation instructions"
echo "  Add the following CNAME records to GoDaddy DNS."
echo "  GoDaddy note: Enter the subdomain prefix ONLY"
echo "  (e.g., '_abc123', not '_abc123.ptodd.org')"
echo ""
echo "  Domain: ptodd.org"
echo "    CNAME Name:  $PREFIX_PTODD"
echo "    CNAME Value: $VALUE_PTODD"
echo ""
echo "  Domain: www.ptodd.org"
echo "    CNAME Name:  $PREFIX_WWW"
echo "    CNAME Value: $VALUE_WWW"

# ─── Step 4: Verification command ─────────────────────────────────────────────

echo ""
echo "==> After adding CNAMEs, verify with:"
echo "  aws acm describe-certificate --certificate-arn $CERT_ARN --region us-east-1 --profile kiss"

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "==> ACM certificate ready for validation"
echo "  Domain:          $DOMAIN"
echo "  SAN:             $SAN"
echo "  Certificate ARN: $CERT_ARN"
echo "  Status:          Pending validation (add CNAMEs to GoDaddy)"
echo ""
echo "Certificate ARN: $CERT_ARN"
