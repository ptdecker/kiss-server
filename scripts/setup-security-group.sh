#!/usr/bin/env bash
# setup-security-group.sh
#
# Idempotent script to restrict EC2 port 80 inbound to CloudFront IPs only. Adds CloudFront 
# managed prefix list rule, verifies HTTPS, then removes 0.0.0.0/0.
#
# IMPORTANT: The order is locked and irreversible (see STATE.md):
#   1. Add CloudFront prefix list rule
#   2. Verify HTTPS still works
#   3. Remove 0.0.0.0/0
#
# Usage: bash scripts/setup-security-group.sh
# Requirements: AWS CLI v2 configured with the 'kiss' profile

set -euo pipefail

PROFILE="kiss"
REGION="us-east-1"
SG_NAME="kiss-server-sg"
VERIFY_URL="https://www.ptodd.org/"
ELASTIC_IP="54.83.192.65"

# ─── Step 1: Look up CloudFront prefix list ID ────────────────────────────────

echo "==> Step 1: Look up CloudFront prefix list ID"
PREFIX_LIST_ID=$(aws ec2 describe-managed-prefix-lists \
  --region "$REGION" \
  --profile "$PROFILE" \
  --filters "Name=prefix-list-name,Values=com.amazonaws.global.cloudfront.origin-facing" \
  --query "PrefixLists[0].PrefixListId" \
  --output text)

if [ -z "$PREFIX_LIST_ID" ] || [ "$PREFIX_LIST_ID" = "None" ]; then
  echo "ERROR: Could not find CloudFront managed prefix list in $REGION"
  exit 1
fi
echo "  Prefix list ID: $PREFIX_LIST_ID"

# ─── Step 2: Look up security group ID ────────────────────────────────────────

echo "==> Step 2: Look up security group ID"
SG_ID=$(aws ec2 describe-security-groups \
  --region "$REGION" --profile "$PROFILE" \
  --group-names "$SG_NAME" \
  --query "SecurityGroups[0].GroupId" --output text)
echo "  Security group: $SG_NAME ($SG_ID)"

# ─── Step 3: Check if prefix list rule already exists ─────────────────────────

echo "==> Step 3: Check for existing prefix list rule"
EXISTING_PL_RULE=$(aws ec2 describe-security-group-rules \
  --region "$REGION" --profile "$PROFILE" \
  --filters "Name=group-id,Values=$SG_ID" \
  --query "SecurityGroupRules[?IsEgress==\`false\` && IpProtocol=='tcp' && FromPort==\`80\` && PrefixListId=='$PREFIX_LIST_ID'].SecurityGroupRuleId | [0]" \
  --output text 2>/dev/null || echo "")

# ─── Step 4: Add prefix list rule ─────────────────────────────────────────────

if [ -z "$EXISTING_PL_RULE" ] || [ "$EXISTING_PL_RULE" = "None" ]; then
  echo "==> Step 4: Add CloudFront prefix list rule to port 80"
  aws ec2 authorize-security-group-ingress \
    --region "$REGION" \
    --profile "$PROFILE" \
    --group-name "$SG_NAME" \
    --ip-permissions '[{"IpProtocol":"tcp","FromPort":80,"ToPort":80,"PrefixListIds":[{"PrefixListId":"'"$PREFIX_LIST_ID"'","Description":"CloudFront origin-facing IPs"}]}]'
  echo "  Added prefix list rule"
else
  echo "==> Step 4: Prefix list rule already exists (skipping)"
fi

# ─── Step 5: Verify HTTPS still works ─────────────────────────────────────────
# LOCKED ORDER per D-05 — must run BEFORE removing 0.0.0.0/0

echo "==> Step 5: Verify HTTPS via $VERIFY_URL"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$VERIFY_URL")
if [ "$HTTP_CODE" != "200" ]; then
  echo "ERROR: HTTPS verification failed (HTTP $HTTP_CODE). NOT removing 0.0.0.0/0 rule."
  echo "  The prefix list rule was added but the open rule is preserved as a safety net."
  exit 1
fi
echo "  HTTPS returned $HTTP_CODE (OK)"

# ─── Step 6: Check if 0.0.0.0/0 rule still exists ────────────────────────────

echo "==> Step 6: Check for existing 0.0.0.0/0 rule on port 80"
OPEN_RULE=$(aws ec2 describe-security-group-rules \
  --region "$REGION" --profile "$PROFILE" \
  --filters "Name=group-id,Values=$SG_ID" \
  --query "SecurityGroupRules[?IsEgress==\`false\` && IpProtocol=='tcp' && FromPort==\`80\` && CidrIpv4=='0.0.0.0/0'].SecurityGroupRuleId | [0]" \
  --output text 2>/dev/null || echo "")

# ─── Step 7: Remove 0.0.0.0/0 rule ───────────────────────────────────────────

if [ -n "$OPEN_RULE" ] && [ "$OPEN_RULE" != "None" ]; then
  echo "==> Step 7: Remove 0.0.0.0/0 rule from port 80"
  aws ec2 revoke-security-group-ingress \
    --region "$REGION" \
    --profile "$PROFILE" \
    --group-name "$SG_NAME" \
    --protocol tcp \
    --port 80 \
    --cidr 0.0.0.0/0
  echo "  Removed 0.0.0.0/0 rule"
else
  echo "==> Step 7: 0.0.0.0/0 rule already removed (skipping)"
fi

# ─── Step 8: Verify direct EC2 IP is rejected ────────────────────────────────

echo "==> Step 8: Verify direct EC2 access is rejected"
DIRECT_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "http://$ELASTIC_IP/" 2>/dev/null || true)
if [ "$DIRECT_CODE" = "200" ]; then
  echo "ERROR: Direct EC2 access still returns 200. Security group rule may not have taken effect."
  exit 1
fi
echo "  Direct EC2 access returned $DIRECT_CODE (expected non-200 — PASS)"

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "=== Security Group Update Complete ==="
echo "  Security group:    $SG_NAME ($SG_ID)"
echo "  CloudFront prefix: $PREFIX_LIST_ID"
echo "  HTTPS verified:    $VERIFY_URL -> 200"
echo "  Direct EC2:        http://$ELASTIC_IP/ -> rejected"
echo ""
echo "Port 80 is now restricted to CloudFront IPs only."
