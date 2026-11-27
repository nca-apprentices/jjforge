# Signs the people of the seed in the way a browser does, one redirect at a
# time, so the scenarios get real tokens. docs/specs/6-human-login.md gives
# each step. `mise run e2e` and e2e/cli/forge.bash source this file.
#
# The twin's login form takes the login name as `username`, so posting it
# stands in for a person who types their GitHub password.

sign_in_endpoint=${JJFORGE_ENDPOINT:-http://localhost:${JJFORGE_PORT:-8080}}

# Prints where a request redirects to, and fails without a redirect.
location_of() {
  local location
  location=$(curl -sS -o /dev/null -w '%{redirect_url}' "$@") || return
  [[ -n $location ]] || return
  echo "$location"
}

# Prints the value of the query parameter $2 in the address $1.
query_param() {
  local value=${1#*[?&]"$2"=}
  [[ $value != "$1" ]] || return
  echo "${value%%&*}"
}

# Signs a person in through the browser flow as the web app does, and prints
# the token answer as JSON, such as `sign_in_json acme-owner acme`.
sign_in_json() {
  local login=$1 org=$2 verifier challenge location code
  # PKCE: the client keeps the verifier and sends only its hash.
  verifier=$(openssl rand -hex 32)
  challenge=$(printf %s "$verifier" | openssl dgst -sha256 -binary | openssl base64 -A | tr '+/' '-_' | tr -d '=')
  # 1. jjforge sends the browser to the organization's provider.
  location=$(location_of -G "$sign_in_endpoint/oauth/authorize" \
    -d response_type=code -d client_id=web -d "redirect_uri=$sign_in_endpoint/callback" \
    -d state=e2e -d "org=$org" -d "code_challenge=$challenge" \
    -d code_challenge_method=S256) || return
  # 2. The person signs in at the provider, which sends the browser back.
  location=$(location_of -d "username=$login" "$location") || return
  # 3. jjforge reads who signed in and sends the browser to the client.
  location=$(location_of "$location") || return
  code=$(query_param "$location" code) || return
  # 4. The client trades the code and its verifier for tokens.
  curl -fsS "$sign_in_endpoint/oauth/token" -d grant_type=authorization_code \
    -d client_id=web -d "code=$code" -d "redirect_uri=$sign_in_endpoint/callback" \
    -d "code_verifier=$verifier"
}

# Prints an access token of a person, such as `sign_in acme-owner acme`.
sign_in() {
  sign_in_json "$@" | jq -r .access_token
}

# Confirms a device sign-in in place of the person's browser, such as
# `confirm_device WDJB-MJHT acme-member`, and prints where it ends.
confirm_device() {
  local user_code=$1 login=$2 location
  location=$(location_of "$sign_in_endpoint/oauth/device/confirm" -d "user_code=$user_code") || return
  location=$(location_of -d "username=$login" "$location") || return
  location_of "$location"
}
