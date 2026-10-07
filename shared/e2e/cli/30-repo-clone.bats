# pending
# A person clones any repository they can read, and the clone keeps change
# IDs, conflicts, and the repository's operations.

bats_require_minimum_version 1.5.0
load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo clone)
  as acme_owner jf clone "$url" owner
  echo hello >owner/README
  as acme_owner jf -R owner describe -m "first"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
  change=$(jf -R owner log -r main --no-graph -T change_id)
}

@test "30.3 a member clones the pushed change with its change ID" {
  run as acme_member jf clone "$url" member
  [ "$status" -eq 0 ]
  cd member
  [ "$(jf log -r main --no-graph -T change_id)" = "$change" ]
  [ "$(jf log -r main --no-graph -T description)" = "first" ]
}

@test "30.2 the clone holds the repository's operations" {
  as acme_member jf clone "$url" member
  cd member
  run jf op log --no-graph -T 'description ++ "\n"'
  [ "$status" -eq 0 ]
  [[ $output == *"push"* ]]
}

@test "30.5 a person outside the organization gets not found" {
  run --separate-stderr as other_owner jf clone "$url" outsider
  [ "$status" -eq 4 ]
  [ ! -e outsider ]
}
