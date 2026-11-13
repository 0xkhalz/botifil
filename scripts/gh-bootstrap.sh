#!/usr/bin/env bash
set -euo pipefail

REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
echo "Bootstrapping GitHub settings for $REPO..."

# 1. Labels (19 total across 4 groups)
while IFS='|' read -r name color desc; do
  [[ -z "$name" || "$name" =~ ^# ]] && continue
  name=$(echo "$name" | xargs)
  color=$(echo "$color" | xargs)
  desc=$(echo "$desc" | xargs)
  gh label create "$name" --color "$color" --description "$desc" --force --repo "$REPO"
done << 'EOF'
type:feat          | 1d76db | New feature
type:fix           | 1d76db | Bug fix
type:docs          | 1d76db | Documentation only
type:chore         | 1d76db | Routine task or maintenance
type:refactor      | 1d76db | Code change without fixing bugs or adding features
type:test          | 1d76db | Test suite additions or corrections
type:perf          | 1d76db | Performance improvement
area:bot           | 5319e7 | Telegram bot logic
area:ci            | 5319e7 | CI/CD pipelines and automation
area:docs          | 5319e7 | Documentation and guides
area:agents        | 5319e7 | Agent specifications and rules
status:needs-spec  | fbca04 | Requires specification before implementation
status:ready       | fbca04 | Ready to be picked up
status:in-progress | fbca04 | Currently being worked on
status:blocked     | fbca04 | Blocked by dependencies or questions
status:needs-human | fbca04 | Blocked awaiting human review or action
size:S             | 0e8a16 | <=100 LoC
size:M             | 0e8a16 | <=300 LoC
size:L             | 0e8a16 | Must split (>300 LoC)
EOF

# 2. Ruleset
echo "Applying branch ruleset..."
RULESET_ID=$(gh api "repos/$REPO/rulesets" --jq 'first(.[] | select(.name == "main-protection") | .id) // empty')

if [[ -n "$RULESET_ID" ]]; then
  echo "Updating existing ruleset id $RULESET_ID..."
  gh api -X PUT "repos/$REPO/rulesets/$RULESET_ID" --input .github/rulesets/main.json
else
  echo "Creating new ruleset..."
  gh api -X POST "repos/$REPO/rulesets" --input .github/rulesets/main.json
fi

echo "gh-bootstrap: OK"
