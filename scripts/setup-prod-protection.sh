#!/usr/bin/env bash
set -euo pipefail

# setup-prod-protection.sh
# Idempotent script to create or update the "Protect prod" ruleset
# on ptdecker/kiss-server via the GitHub Rulesets API.
#
# Usage: bash scripts/setup-prod-protection.sh
# Requirements: gh CLI authenticated as ptdecker

REPO="ptdecker/kiss-server"
RULESET_NAME="Protect prod"

echo "Checking for existing ruleset named '${RULESET_NAME}'..."

EXISTING_ID=$(rtk proxy gh api "/repos/${REPO}/rulesets" \
  | python3 -c "import sys,json; rules=json.load(sys.stdin); \
    match=[r['id'] for r in rules if r['name']=='${RULESET_NAME}']; \
    print(match[0] if match else '')")

TMPFILE=$(mktemp)

cat > "$TMPFILE" << 'ENDJSON'
{
  "name": "Protect prod",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["refs/heads/prod"],
      "exclude": []
    }
  },
  "bypass_actors": [
    {
      "actor_type": "RepositoryRole",
      "actor_id": 5,
      "bypass_mode": "always"
    }
  ],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" }
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
echo "Done. Ruleset 'Protect prod' is active."
