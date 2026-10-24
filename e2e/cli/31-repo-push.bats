# pending
# A person pushes bookmarks and changes, and the forge keeps their change IDs,
# their conflicts, and the earlier versions of each change.

bats_require_minimum_version 1.5.0
load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo push)
  as acme_owner jf clone "$url" owner
  cd owner
}

@test "31.2 a pushed change keeps its change ID" {
  as acme_owner jf describe -m "first"
  as acme_owner jf bookmark create main -r @
  change=$(jf log -r main --no-graph -T change_id)

  run as acme_owner jf push --bookmark main
  [ "$status" -eq 0 ]

  as acme_member jf clone "$url" ../member
  [ "$(jf -R ../member log -r main --no-graph -T change_id)" = "$change" ]
}

@test "31.3 a pushed conflict stays a conflict" {
  echo base >file
  as acme_owner jf new -m "left"
  echo left >file
  as acme_owner jf new -m "right" 'root()+'
  echo right >file
  as acme_owner jf rebase -r @ -d 'description("left")'
  as acme_owner jf bookmark create conflicted -r @

  run as acme_owner jf push --bookmark conflicted
  [ "$status" -eq 0 ]

  as acme_member jf clone "$url" ../member
  [ "$(jf -R ../member log -r conflicted --no-graph -T conflict)" = "true" ]
}
