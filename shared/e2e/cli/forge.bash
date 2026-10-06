# Helpers for the jf scenarios. The tokens come from the seed that
# `mise run up` loads once #6 and #13 are built.

# shellcheck source=../http/vars.env
source "$BATS_TEST_DIRNAME/../http/vars.env"

# Creates a repository in `acme` as its owner and prints its clone address.
# The name is unique per run, so the suite runs twice on one server.
new_repo() {
  curl -fsS -X POST "$JJFORGE_ENDPOINT/api/v1/orgs/acme/repos" \
    -H "Authorization: Bearer $acme_owner" \
    -H 'content-type: application/json' \
    -d "{\"name\": \"$1-$$-$RANDOM\"}" | jq -r .cloneUrl
}

# Runs a command as a person of the seed, such as `as acme_member jf push`.
as() {
  local person=$1
  shift
  JJFORGE_TOKEN=${!person} JJ_USER=$person JJ_EMAIL=$person@example.com "$@"
}
