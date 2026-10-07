# pending
# A clone shows every earlier version of each pushed change, so a person sees
# how a change evolved before it reached the forge.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo evolution)
  as acme_owner jf clone "$url" owner
}

@test "103.3 a clone shows each earlier version of a change" {
  as acme_owner jf -R owner describe -m "draft"
  as acme_owner jf -R owner describe -m "reviewed"
  as acme_owner jf -R owner describe -m "final"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main

  as acme_member jf clone "$url" member
  run jf -R member evolog -r main --no-graph -T 'description ++ "\n"'
  [ "$status" -eq 0 ]
  [[ $output == *final*reviewed*draft* ]]
}
