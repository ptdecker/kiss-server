#!/usr/bin/env bash
# setup-branch-protection.sh
#
# Idempotent script to create or update the "Protect main" ruleset on ptdecker/kiss-server via 
# the GitHub Rulesets API.
#
# Usage: bash scripts/setup-branch-protection.sh
# Requirements: gh CLI authenticated as ptdecker

set -euo pipefail

REPO="ptdecker/kiss-server"
RULESET_NAME="Protect main"

echo "Checking for existing ruleset named '${RULESET_NAME}'..."

EXISTING_ID=$(rtk proxy gh api "/repos/${REPO}/rulesets" \
  | python3 -c "import sys,json; rules=json.load(sys.stdin); \
    match=[r['id'] for r in rules if r['name']=='${RULESET_NAME}']; \
    print(match[0] if match else '')")

TMPFILE=$(mktemp)

cat > "$TMPFILE" << 'ENDJSON'
{
  "name": "Protect main",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["refs/heads/main"],
      "exclude": []
    }
  },
  "bypass_actors": [
    {
      "actor_id": 5,
      "actor_type": "RepositoryRole",
      "bypass_mode": "pull_request"
    }
  ],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 1,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "allowed_merge_methods": ["squash"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "required_status_checks": [
          {
            "context": "ci",
            "integration_id": 15368
          }
        ],
        "strict_required_status_checks_policy": true,
        "do_not_enforce_on_create": false
      }
    }
  ]
}
ENDJSON

if [ -n "$EXISTING_ID" ]; then
  echo "Found existing ruleset id=${EXISTING_ID}. Updating via PUT..."
  rtk proxy gh api --method PUT --input "$TMPFILE" "/repos/${REPO}/rulesets/${EXISTING_ID}"
else
  echo "No existing ruleset found. Creating via POST..."
  rtk proxy gh api --method POST --input "$TMPFILE" "/repos/${REPO}/rulesets"
fi

rm -f "$TMPFILE"
echo "Done. Ruleset 'Protect main' is active."
