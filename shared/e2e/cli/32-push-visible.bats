# pending
# A read that starts after a successful push sees the pushed state.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo visible)
  as acme_owner jf clone "$url" owner
}

@test "32.1 a clone right after the push sees the moved bookmark" {
  cd owner
  for n in 1 2 3; do
    as acme_owner jf new -m "change $n"
    as acme_owner jf bookmark set main -r @
    as acme_owner jf push --bookmark main
    pushed=$(jf log -r main --no-graph -T commit_id)

    as acme_member jf clone "$url" "../member-$n"
    [ "$(jf -R "../member-$n" log -r main --no-graph -T commit_id)" = "$pushed" ]
  done
}
