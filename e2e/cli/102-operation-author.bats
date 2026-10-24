# pending
# Every operation in a repository's history names the person who made it, as
# the forge verified them. A client can't claim another name.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo author)
  as acme_member jf clone "$url" member
}

@test "102.1 a push names the verified person, not the claimed one" {
  as acme_member jf -R member describe -m "first"
  as acme_member jf -R member bookmark create main -r @
  as acme_member env JJ_USER=mallory jf -R member push --bookmark main

  as acme_owner jf clone "$url" check
  run jf -R check op log --no-graph -n 1 -T user
  [ "$status" -eq 0 ]
  [[ $output == acme-member* ]]
}
