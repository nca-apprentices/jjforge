# pending
# When two pushes to one repository race, both end up in the repository, or
# the later one is refused with the reason and nothing is lost.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo race)
  as acme_owner jf clone "$url" owner
  as acme_member jf clone "$url" member
}

@test "89.3 two pushes of different bookmarks both land" {
  for n in 1 2 3 4 5; do
    as acme_owner jf -R owner new -m "owner $n"
    as acme_owner jf -R owner bookmark set owner -r @
    as acme_member jf -R member new -m "member $n"
    as acme_member jf -R member bookmark set member -r @

    as acme_owner jf -R owner push --bookmark owner &
    as acme_member jf -R member push --bookmark member &
    wait %1
    wait %2
  done

  as acme_owner jf clone "$url" check
  [ "$(jf -R check log -r owner --no-graph -T description)" = "owner 5" ]
  [ "$(jf -R check log -r member --no-graph -T description)" = "member 5" ]
}
