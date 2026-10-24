# pending
# A clone shows every version of a pushed change that the forge saw. A
# version that never left the owner's clone stays private.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo evolution)
  as acme_owner jf clone "$url" owner
}

@test "103.2 a version that was never pushed stays in the owner's clone" {
  echo "token=hunter2" >owner/config
  as acme_owner jf -R owner describe -m "draft"
  echo "token=redacted" >owner/config
  as acme_owner jf -R owner describe -m "final"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main

  as acme_member jf clone "$url" member
  run jf -R member evolog -r main --no-graph -T 'description ++ "\n"'
  [ "$status" -eq 0 ]
  [[ $output != *draft* ]]
  run jf -R member log -r 'all()' --patch
  [ "$status" -eq 0 ]
  [[ $output != *hunter2* ]]
}

@test "103.3 a clone shows each pushed version of a change" {
  as acme_owner jf -R owner describe -m "draft"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
  as acme_owner jf -R owner describe -m "reviewed"
  as acme_owner jf -R owner push --bookmark main
  as acme_owner jf -R owner describe -m "final"
  as acme_owner jf -R owner push --bookmark main

  as acme_member jf clone "$url" member
  run jf -R member evolog -r main --no-graph -T 'description ++ "\n"'
  [ "$status" -eq 0 ]
  [[ $output == *final*reviewed*draft* ]]
}
