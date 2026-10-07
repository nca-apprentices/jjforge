# pending
# After every forge service restarts, a repository pushed before the restart
# clones back whole. It restarts the services of `mise run up`.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo restart)
  as acme_owner jf clone "$url" owner
  as acme_owner jf -R owner describe -m "before the restart"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
  change=$(jf -R owner log -r main --no-graph -T change_id)
}

@test "97.3 a clone after a restart holds the pushed change" {
  mise run compose restart
  until curl -fsS "$JJFORGE_ENDPOINT/actuator/health" >/dev/null; do sleep 1; done

  as acme_member jf clone "$url" member
  [ "$(jf -R member log -r main --no-graph -T change_id)" = "$change" ]
}
