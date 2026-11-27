# pending
# vcs checks a token against keys it already holds, so a person who signed
# in keeps reading repositories while the server, which signs people in, is
# down. It stops the server of `mise run up` and starts it again.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(vcs_repo)
  as acme_owner jf clone "$url" owner
  as acme_owner jf -R owner describe -m "before the outage"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
  change=$(jf -R owner log -r main --no-graph -T change_id)
  token=$(sign_in acme-member acme)
  mise run compose stop server
}

teardown() {
  mise run compose start server
  until curl -fsS "$JJFORGE_ENDPOINT/actuator/health" >/dev/null; do sleep 1; done
}

@test "12.1 a clone works while the server is down" {
  run curl -fsS "$JJFORGE_ENDPOINT/oauth/keys"
  [ "$status" -ne 0 ]

  JJFORGE_TOKEN=$token jf clone "$url" member
  [ "$(jf -R member log -r main --no-graph -T change_id)" = "$change" ]
}

@test "12.2 a fetch works while the server is down" {
  mise run compose start server
  until curl -fsS "$JJFORGE_ENDPOINT/actuator/health" >/dev/null; do sleep 1; done
  JJFORGE_TOKEN=$token jf clone "$url" member
  mise run compose stop server

  JJFORGE_TOKEN=$token jf -R member fetch
  [ "$(jf -R member log -r main@forge --no-graph -T change_id)" = "$change" ]
}
