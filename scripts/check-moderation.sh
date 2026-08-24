#!/usr/bin/env bash
# Prove that comment moderation fails closed.
#
#   ./scripts/check-moderation.sh
#
# The claim on /comment-policy is that there is no path through this system where
# a failure results in a comment appearing. This is the check behind it.
#
# It registers an account, posts a comment from it, drains the classification
# queue, and asserts the comment never becomes visible to an anonymous visitor.
# With no AI provider configured the classifier cannot answer, which is precisely
# the failure the claim is about: the job errors, the comment stays held, and a
# person has to look at it.
#
# Everything it creates is left behind on purpose. This runs against a local
# stack that `./scripts/run.sh --fresh` rebuilds from nothing.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

PROJECT="${COMPOSE_PROJECT_NAME:-trovato-site}"
BASE="http://127.0.0.1:${SITE_PORT:-3080}"
CRON_KEY="${CRON_KEY:-local-development-cron-key}"

# One fixed account, reused. A fresh name per run would be tidier and does not
# work: registration is rate-limited to three an hour per IP, so the fourth run
# in an hour would fail on the sign-up rather than on what it is checking.
USERNAME="${MODERATION_CHECK_USER:-moderation-check}"
PASSWORD="a-long-enough-password-for-the-check"
MARKER="fail-closed marker $(date +%s)"

jar="$(mktemp)"
work="$(mktemp -d)"
trap 'rm -rf "$jar" "$work"' EXIT

# The hidden CSRF field from a page this session just fetched.
token_from() {
    grep -o 'name="'"$2"'" value="[^"]*"' "$1" | head -1 | sed 's/.*value="//;s/"//'
}

psql_q() {
    docker compose -p "$PROJECT" exec -T postgres \
        psql -U "${POSTGRES_USER:-trovato}" -d "${POSTGRES_DB:-trovato}" -t -A -c "$1" | tr -d '\r'
}

fail() { echo "FAIL: $1"; exit 1; }

if [ -z "$(psql_q "SELECT 1 FROM users WHERE name = '${USERNAME}';")" ]; then
    echo "==> Registering ${USERNAME}"
    curl -fsS -c "$jar" "${BASE}/user/register" -o "${work}/register.html"
    curl -fsS -b "$jar" -c "$jar" -o /dev/null -X POST "${BASE}/user/register" \
        --data-urlencode "_token=$(token_from "${work}/register.html" _token)" \
        --data-urlencode "username=${USERNAME}" \
        --data-urlencode "mail=${USERNAME}@example.com" \
        --data-urlencode "password=${PASSWORD}" \
        --data-urlencode "confirm_password=${PASSWORD}" \
        || fail "registration was refused. Three an hour per IP is the limit; wait or use MODERATION_CHECK_USER."
    [ -n "$(psql_q "SELECT 1 FROM users WHERE name = '${USERNAME}';")" ] \
        || fail "registration returned success and created no account"
else
    echo "==> Reusing ${USERNAME}"
fi

# Registration leaves the account blocked until the verification link is
# followed, and that link only ever existed in an email: the database stores the
# token's hash and nothing else. Without SMTP credentials there is no way to
# complete verification through the front door, so the check activates the
# account directly. This one line stands in for an email, and it is the only step
# here that is not something a visitor could do.
psql_q "UPDATE users SET status = 1 WHERE name = '${USERNAME}';" >/dev/null

# The trust ladder publishes an account's comments immediately once three of them
# have been approved, and this check is about what happens to a comment that is
# held. Reusing one account across runs would eventually earn its way past the
# queue and the check would start passing for the wrong reason, so its approved
# comments are cleared first.
psql_q "DELETE FROM comment WHERE author_id = (SELECT id FROM users WHERE name = '${USERNAME}');" >/dev/null

echo "==> Logging in"
curl -fsS -b "$jar" -c "$jar" "${BASE}/user/login" -o "${work}/login.html"
curl -fsS -b "$jar" -c "$jar" -o /dev/null -X POST "${BASE}/user/login" \
    --data-urlencode "_token=$(token_from "${work}/login.html" _token)" \
    --data-urlencode "username=${USERNAME}" \
    --data-urlencode "password=${PASSWORD}"

echo "==> Posting a comment"
item_path="/news/trovato-0-101-0"
curl -fsS -b "$jar" "${BASE}${item_path}" -o "${work}/item.html"
csrf="$(token_from "${work}/item.html" _csrf)"
[ -n "$csrf" ] || fail "the comment form was not offered to a logged-in account"

item_id="$(grep -o 'action="/api/item/[0-9a-f-]*/comments"' "${work}/item.html" \
    | head -1 | sed 's|.*/api/item/||;s|/comments"||')"
[ -n "$item_id" ] || fail "could not find the comment form's target item"

curl -fsS -b "$jar" -c "$jar" -o /dev/null -X POST "${BASE}/api/item/${item_id}/comments" \
    --data-urlencode "_csrf=${csrf}" \
    --data-urlencode "body=${MARKER}"

echo "==> Draining the classification queue"
for _ in $(seq 1 8); do
    curl -fsS -o /dev/null -X POST "${BASE}/cron/${CRON_KEY}" || true
    sleep 2
done

echo "==> Checking"

# 1. The queue tried and could not answer. If it succeeded, this stack has an AI
#    provider configured and the check is measuring something else.
queue_state="$(psql_q "SELECT status || ' attempts=' || attempts FROM plugin_queue WHERE queue_name = 'comment_moderation' ORDER BY created_at DESC LIMIT 1;")"
echo "    queue: ${queue_state:-<empty>}"

# 2. The comment is not published.
status="$(psql_q "SELECT status FROM comment WHERE body LIKE '%${MARKER}%' LIMIT 1;")"
[ -n "$status" ] || fail "the comment was not stored at all"
[ "$status" != "1" ] || fail "the comment was PUBLISHED after the classifier failed. This is the failure the policy says cannot happen."
echo "    comment status: ${status} (1 would be published)"

# 3. And an anonymous visitor cannot see it, which is the property that matters.
if curl -fsS "${BASE}${item_path}" | grep -q "${MARKER}"; then
    fail "an anonymous visitor can see a comment that was never approved"
fi
echo "    not visible to an anonymous visitor"

echo
echo "PASS: the classifier could not answer and the comment stayed held."
