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
}

credentials() {
  echo "$XDG_CONFIG_HOME/jjforge/credentials.toml"
}

@test "161.1 jf login signs a person in on another device and says who" {
  # jf has no terminal here, so it opens no browser and prints the code.
  jf login --org acme 2>login.err &
  local pid=$! code=""
  for _ in $(seq 30); do
    code=$(grep -oE '[A-Z]{4}-[A-Z]{4}' login.err || true)
    [[ -n $code ]] && break
    sleep 1
  done
  [[ -n $code ]]
  grep -q "$JJFORGE_ENDPOINT/device" login.err

  confirm_device "$code" acme-owner
  wait "$pid"
  grep -q "Signed in as acme-owner" login.err
}

@test "161.2 the credentials file is readable by its owner only" {
  [ -n "$(find "$(credentials)" -perm 600)" ]
}

@test "161.3 a clone and a push need no token in the environment" {
  unset JJFORGE_TOKEN
  jf clone "$url" work
  jf -R work describe -m "first"
  jf -R work bookmark create main -r @
  jf -R work push --bookmark main
}

@test "161.4 after the access token expires, jf refreshes it and pushes" {
  local before
  before=$(cat "$(credentials)")
  sleep 66

  unset JJFORGE_TOKEN
  jf -R work new -m "second"
  jf -R work bookmark set main -r @
  jf -R work push --bookmark main
  [ "$(cat "$(credentials)")" != "$before" ]
}

@test "161.5 jf logout ends the session, and the next push exits with 5" {
  unset JJFORGE_TOKEN
  jf logout
  [ ! -e "$(credentials)" ]

  jf -R work new -m "third"
  jf -R work bookmark set main -r @
  run jf -R work push --bookmark main
  [ "$status" -eq 5 ]
  [[ $output == *"jf login"* ]]
}
