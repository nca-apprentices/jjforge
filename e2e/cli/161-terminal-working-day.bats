# pending
# A person signs in once with `jf login`, and `jf` keeps them signed in for
# the rest of the session by refreshing its token in the background. Compose
# gives an access token 60 seconds, so the file waits for one to expire.
# Each test continues where the one before it stopped.

load forge

setup_file() {
  export XDG_CONFIG_HOME="$BATS_FILE_TMPDIR/config"
  url=$(vcs_repo)
  export url
}

setup() {
  cd "$BATS_FILE_TMPDIR"
  unset JJFORGE_TOKEN
}

credentials() {
  echo "$XDG_CONFIG_HOME/jjforge/credentials.toml"
}

# Starts `jf login --org acme` in the background, writing its stderr to
# login.err, and sets login_pid and, once jf shows it, user_code.
start_login() {
  jf login --org acme >login.out 2>login.err &
  login_pid=$!
  user_code=""
  for _ in $(seq 30); do
    user_code=$(grep -oE '[A-Z]{4}-[A-Z]{4}' login.err || true)
    [[ -n $user_code ]] && return
    sleep 1
  done
  return 1
}

# Prints the refresh token of the saved session.
saved_refresh_token() {
  grep -oE 'jfr_[A-Za-z0-9_-]+' "$(credentials)"
}

@test "161.1 jf login signs a person in on another device and says who" {
  # jf has no terminal here, so it opens no browser and prints the code.
  start_login
  grep -q "$JJFORGE_ENDPOINT/device" login.err

  confirm_device "$user_code" acme-owner
  wait "$login_pid"
  grep -q "Signed in as acme-owner" login.err
}

@test "161.2 the credentials file is readable by its owner only" {
  [ -n "$(find "$(credentials)" -perm 600)" ]
}

@test "161.3 a clone and a push need no token in the environment" {
  jf clone "$url" work
  jf -R work describe -m "first"
  jf -R work bookmark create main -r @
  jf -R work push --bookmark main
}

@test "161.3 after the access token expires, jf refreshes it and pushes" {
  local before
  before=$(cat "$(credentials)")
  sleep 66

  jf -R work new -m "second"
  jf -R work bookmark set main -r @
  jf -R work push --bookmark main
  [ "$(cat "$(credentials)")" != "$before" ]
}

@test "161.6 jf logout ends the session and deletes it" {
  local refresh
  refresh=$(saved_refresh_token)
  jf logout
  [ ! -e "$(credentials)" ]

  run curl -sS "$JJFORGE_ENDPOINT/oauth/token" -d grant_type=refresh_token \
    -d client_id=jf -d "refresh_token=$refresh"
  [ "$(jq -r .error <<<"$output")" = invalid_grant ]
}

@test "161.9 without a session, a push exits with 5 and names jf login" {
  jf -R work new -m "third"
  jf -R work bookmark set main -r @
  run jf -R work push --bookmark main
  [ "$status" -eq 5 ]
  [[ $output == *"jf login --org"* ]]
}

@test "161.4 a session that ended elsewhere exits with 5 at the next refresh" {
  start_login
  confirm_device "$user_code" acme-owner
  wait "$login_pid"
  curl -fsS "$JJFORGE_ENDPOINT/oauth/revoke" -d "token=$(saved_refresh_token)"
  sleep 66

  run jf -R work push --bookmark main
  [ "$status" -eq 5 ]
  [[ $output == *"Your session ended. Run jf login --org acme."* ]]
  [ ! -e "$(credentials)" ]
}

@test "161.8 jf login exits with 5 when the person declines, and saves nothing" {
  start_login
  decline_device "$user_code"
  run wait "$login_pid"
  [ "$status" -eq 5 ]
  [ ! -e "$(credentials)" ]
}
