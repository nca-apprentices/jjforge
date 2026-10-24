# pending
# A push either applies whole or not at all.

bats_require_minimum_version 1.5.0
load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo atomic)
  as acme_owner jf clone "$url" owner
  cd owner
  as acme_owner jf describe -m "first"
  as acme_owner jf bookmark create main -r @
  as acme_owner jf push --bookmark main
}

@test "33.3 a refused push changes nothing" {
  as acme_owner jf new -m "second"
  as acme_owner jf bookmark set main -r @
  as acme_owner jf bookmark create feature -r @

  run --separate-stderr as other_owner jf push --bookmark main --bookmark feature
  [ "$status" -eq 4 ]

  as acme_member jf clone "$url" ../member
  [ "$(jf -R ../member log -r main --no-graph -T description)" = "first" ]
  run jf -R ../member log -r feature
  [ "$status" -ne 0 ]
}
