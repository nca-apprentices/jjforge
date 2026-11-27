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

@test "12.4 a token signed under a key ID that the server never published gets 401" {
  local header claims signature
  header=$(printf '{"alg":"EdDSA","typ":"at+jwt","kid":"never-published"}' |
    openssl base64 -A | tr '+/' '-_' | tr -d '=')
  IFS=. read -r _ claims signature <<<"$token"
  run curl -sS -o /dev/null -w '%{http_code}' \
    -H "Authorization: Bearer $header.$claims.$signature" "$url/head"
  [ "$output" = 401 ]
}

@test "12.5 a vcs that starts while the server is down waits for the keys" {
  # A fresh token, so it outlives the restarts below.
  mise run compose start server
  until curl -fsS "$JJFORGE_ENDPOINT/actuator/health" >/dev/null; do sleep 1; done
  token=$(sign_in acme-member acme)
  mise run compose stop server

  mise run compose restart vcs
  until curl -fsS "${url%%/sync/v1/*}/sync/v1/keys" >/dev/null; do sleep 1; done
  run curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $token" "$url/head"
  [ "$output" = 401 ]

  mise run compose start server
  local status_code=""
  for _ in $(seq 30); do
    status_code=$(curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $token" "$url/head")
    [ "$status_code" = 200 ] && break
    sleep 1
  done
  [ "$status_code" = 200 ]
}
