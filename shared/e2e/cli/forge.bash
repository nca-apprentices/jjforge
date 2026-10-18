# Helpers for the jf scenarios. Each person of the seed names themselves in
# JJFORGE_TOKEN, and vcs grants the scopes of shared/deploy/twins/principals,
# until sign-in exists, as #6 plans.

# shellcheck source=../http/vars.env
source "$BATS_TEST_DIRNAME/../http/vars.env"

# The scenarios read no jj configuration of the machine they run on, such as
# `ui.log-word-wrap`, which wraps the lines a scenario parses. `copy` sets
# one per person.
export JJ_CONFIG=/dev/null

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
  JJFORGE_TOKEN=$person JJ_USER=$person JJ_EMAIL=$person@example.com "$@"
}

# The organizations of the seed, by ID, as the principals' scopes name them.
acme_org=0199c9a0-0000-7000-8000-00000000ac3e

# Creates a repository in `acme` through vcs and prints its clone address,
# until the server creates repositories, as #25 decides.
vcs_repo() {
  local repo
  repo=$(uuidgen | tr '[:upper:]' '[:lower:]')
  grpcurl -plaintext -import-path "$BATS_TEST_DIRNAME/../../proto" \
    -proto source/v1/repo.proto \
    -H "authorization: Bearer acme_owner" \
    -d "{\"repo\": {\"org_id\": \"$acme_org\", \"repo_id\": \"$repo\"}}" \
    "localhost:${JJFORGE_VCS_PORT:-50052}" source.v1.RepoService/CreateRepo >/dev/null ||
    return
  echo "http://localhost:${JJFORGE_SYNC_PORT:-8081}/sync/v1/$acme_org/$repo"
}

# Runs a jf copy of its own, with its own configuration, as a person of the
# seed, such as `copy acme_member jf push`.
copy() {
  local person=$1
  shift
  printf '[user]\nname = "%s"\nemail = "%s@example.com"\n' "$person" "$person" \
    >"$BATS_TEST_TMPDIR/$person.toml"
  JJ_CONFIG="$BATS_TEST_TMPDIR/$person.toml" JJFORGE_TOKEN=$person "$@"
}
