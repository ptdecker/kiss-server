#!/usr/bin/env bash
set -euo pipefail

# setup-branch-protection.sh
# Idempotent script to create or update the "Protect main" ruleset
# on ptdecker/kiss-server via the GitHub Rulesets API.
#
# Usage: bash scripts/setup-branch-protection.sh
# Requirements: gh CLI authenticated as ptdecker

REPO="ptdecker/kiss-server"
RULESET_NAME="Protect main"

echo "Checking for existing ruleset named '${RULESET_NAME}'..."

EXISTING_ID=$(rtk proxy gh api "/repos/${REPO}/rulesets" \
  | python3 -c "import sys,json; rules=json.load(sys.stdin); \
    match=[r['id'] for r in rules if r['name']=='${RULESET_NAME}']; \
    print(match[0] if match else '')")

TMPFILE=$(mktemp /tmp/ruleset.XXXXXX.json)

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
      "actor_type": "RepositoryRole",
      "actor_id": 5,
      "bypass_mode": "always"
    }
  ],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
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
  echo "Found existing ruleset id=${EXISTING_ID}. Updating via PATCH..."
  rtk proxy gh api --method PATCH --input "$TMPFILE" "/repos/${REPO}/rulesets/${EXISTING_ID}"
else
  echo "No existing ruleset found. Creating via POST..."
  rtk proxy gh api --method POST --input "$TMPFILE" "/repos/${REPO}/rulesets"
fi

rm -f "$TMPFILE"
echo "Done. Ruleset 'Protect main' is active."
