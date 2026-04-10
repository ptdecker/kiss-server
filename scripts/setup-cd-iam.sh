#!/usr/bin/env bash
set -euo pipefail

# setup-cd-iam.sh
# Idempotent script to create an IAM user for the CD pipeline's CloudFront cache invalidation.
# Creates the kiss-cd-cloudfront IAM user with an inline policy scoped to
# cloudfront:CreateInvalidation on the specific distribution ARN, generates an access key,
# and prints gh secret set commands to store credentials in GitHub Secrets.
#
# Usage: bash scripts/setup-cd-iam.sh DISTRIBUTION_ID
#   Get DISTRIBUTION_ID from: docs/aws-resources.md or scripts/setup-cloudfront.sh output
#
# Requirements: AWS CLI v2 configured with the 'kiss' profile, gh CLI
#
# Resources provisioned:
#   - IAM user: kiss-cd-cloudfront
#   - Inline policy: kiss-cd-cloudfront-invalidation (cloudfront:CreateInvalidation on specific ARN)
#   - IAM access key: printed once — store immediately as GitHub Secrets

REGION="us-east-1"
PROFILE="kiss"
IAM_USER="kiss-cd-cloudfront"
POLICY_NAME="kiss-cd-cloudfront-invalidation"

# ─── Argument validation ───────────────────────────────────────────────────────

DISTRIBUTION_ID="${1:-}"
if [ -z "$DISTRIBUTION_ID" ]; then
  echo "Usage: bash scripts/setup-cd-iam.sh DISTRIBUTION_ID"
  echo "  Get DISTRIBUTION_ID from: docs/aws-resources.md or scripts/setup-cloudfront.sh output"
  exit 1
fi

# ─── Step 1: IAM user (idempotent) ────────────────────────────────────────────

echo "==> Step 1: IAM user"

EXISTING_USER=$(aws iam get-user \
  --user-name "$IAM_USER" \
  --profile "$PROFILE" \
  --query 'User.UserName' \
  --output text 2>/dev/null || echo "")

if [ -n "$EXISTING_USER" ] && [ "$EXISTING_USER" != "None" ]; then
  echo "  IAM user '$IAM_USER' already exists, skipping."
else
  echo "  Creating IAM user '$IAM_USER'..."
  aws iam create-user \
    --user-name "$IAM_USER" \
    --profile "$PROFILE" \
    --output text > /dev/null
  echo "  IAM user '$IAM_USER' created."
fi

# ─── Step 2: Inline policy (put-user-policy is idempotent — creates or replaces) ──

echo "==> Step 2: Inline policy"

ACCOUNT_ID=$(aws sts get-caller-identity \
  --profile "$PROFILE" \
  --query 'Account' \
  --output text)

# CloudFront ARN format: double colon (no region component — CloudFront is global)
DISTRIBUTION_ARN="arn:aws:cloudfront::${ACCOUNT_ID}:distribution/${DISTRIBUTION_ID}"

POLICY_JSON="{
  \"Version\": \"2012-10-17\",
  \"Statement\": [{
    \"Effect\": \"Allow\",
    \"Action\": \"cloudfront:CreateInvalidation\",
    \"Resource\": \"${DISTRIBUTION_ARN}\"
  }]
}"

aws iam put-user-policy \
  --user-name "$IAM_USER" \
  --policy-name "$POLICY_NAME" \
  --policy-document "$POLICY_JSON" \
  --profile "$PROFILE" \
  --output text > /dev/null

echo "  Inline policy '$POLICY_NAME' applied."
echo "  Resource: $DISTRIBUTION_ARN"

# ─── Step 3: Access key ────────────────────────────────────────────────────────

echo "==> Step 3: Access key"

KEY_OUTPUT=$(aws iam create-access-key \
  --user-name "$IAM_USER" \
  --profile "$PROFILE" \
  --output json 2>&1) || {
  echo ""
  echo "  ERROR: Failed to create access key."
  echo "  The IAM user may already have the maximum number of access keys (2)."
  echo "  List existing keys: aws iam list-access-keys --user-name $IAM_USER --profile $PROFILE"
  echo "  Delete an old key:  aws iam delete-access-key --user-name $IAM_USER --access-key-id KEY_ID --profile $PROFILE"
  echo "  Then re-run this script."
  exit 1
}

ACCESS_KEY_ID=$(echo "$KEY_OUTPUT" | python3 -c "import sys,json; print(json.load(sys.stdin)['AccessKey']['AccessKeyId'])")
SECRET_ACCESS_KEY=$(echo "$KEY_OUTPUT" | python3 -c "import sys,json; print(json.load(sys.stdin)['AccessKey']['SecretAccessKey'])")

# ─── Step 4: Print gh secret set commands ─────────────────────────────────────

echo "==> Step 4: GitHub Secrets"

REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || echo "todddecker/kiss-server")

echo ""
echo "==> Access key created (secret shown once — store it now)"
echo "  AWS_ACCESS_KEY_ID:     $ACCESS_KEY_ID"
echo "  AWS_SECRET_ACCESS_KEY: $SECRET_ACCESS_KEY"
echo ""
echo "==> Run these commands to store GitHub Secrets:"
echo "  gh secret set CF_AWS_ACCESS_KEY_ID --body \"$ACCESS_KEY_ID\" --repo $REPO"
echo "  gh secret set CF_AWS_SECRET_ACCESS_KEY --body \"$SECRET_ACCESS_KEY\" --repo $REPO"
echo "  gh secret set CLOUDFRONT_DISTRIBUTION_ID --body \"$DISTRIBUTION_ID\" --repo $REPO"

# ─── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "==> CD IAM provisioning complete"
echo "  IAM User:       $IAM_USER"
echo "  Policy:         $POLICY_NAME"
echo "  Distribution:   $DISTRIBUTION_ID"
echo "  Action scoped:  cloudfront:CreateInvalidation"
