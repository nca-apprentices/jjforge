# pending
# A person with a clone brings in everything pushed since, including moved
# bookmarks and rewritten changes, without cloning again.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo fetch)
  as acme_owner jf clone "$url" owner
  as acme_owner jf -R owner describe -m "first"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
  as acme_member jf clone "$url" member
}

@test "101.2 a fetch brings in a moved bookmark" {
  as acme_owner jf -R owner new -m "second"
  as acme_owner jf -R owner bookmark set main -r @
  as acme_owner jf -R owner push --bookmark main

  run as acme_member jf -R member fetch
  [ "$status" -eq 0 ]
  [ "$(jf -R member log -r main --no-graph -T description)" = "second" ]
}

@test "101.3 a fetch brings in a rewritten change under its change ID" {
  change=$(jf -R owner log -r main --no-graph -T change_id)
  as acme_owner jf -R owner describe -r main -m "first, reworded"
  as acme_owner jf -R owner push --bookmark main

  as acme_member jf -R member fetch
  [ "$(jf -R member log -r "$change" --no-graph -T description)" = "first, reworded" ]
}
