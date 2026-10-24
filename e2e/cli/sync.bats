#!/usr/bin/env bats
# Several jf copies sync through vcs at once, as #108 asks: each with its
# own configuration, signed in as a person of the seed.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(vcs_repo)
  copy acme_owner jf clone "$url" owner
  copy acme_member jf clone "$url" member
}

@test "two copies that push to one repository at once both land" {
  copy acme_owner jf -R owner new -m "from the owner"
  copy acme_owner jf -R owner bookmark create owner -r @
  copy acme_member jf -R member new -m "from the member"
  copy acme_member jf -R member bookmark create member -r @

  copy acme_owner jf -R owner push --bookmark owner &
  copy acme_member jf -R member push --bookmark member &
  wait %1
  wait %2

  copy acme_owner jf clone "$url" check
  [ "$(jf -R check log -r owner --no-graph -T description)" = "from the owner" ]
  [ "$(jf -R check log -r member --no-graph -T description)" = "from the member" ]
}

@test "a clone holds the operation that the push made, under its ID" {
  copy acme_owner jf -R owner new -m "first"
  copy acme_owner jf -R owner bookmark create main -r @
  copy acme_owner jf -R owner push --bookmark main

  copy acme_member jf -R member fetch
  [ "$(jf -R member log -r main --no-graph -T description)" = "first" ]
  pushed=$(jf -R owner op log --no-graph -T 'id ++ " " ++ user ++ "\n"' |
    awk '$2 ~ /@jjforge$/ { print $1; exit }')
  [ -n "$pushed" ]
  jf -R member op log --no-graph -T 'id ++ "\n"' | grep -qx "$pushed"
}

@test "a fetch follows the working copy onto a rewritten parent" {
  echo one >owner/file
  copy acme_owner jf -R owner commit -m "first"
  copy acme_owner jf -R owner bookmark create main -r @-
  copy acme_owner jf -R owner push --bookmark main
  copy acme_member jf -R member fetch
  copy acme_member jf -R member new main

  echo two >owner/file
  copy acme_owner jf -R owner squash --into main
  copy acme_owner jf -R owner push --bookmark main

  copy acme_member jf -R member fetch
  [ "$(cat member/file)" = two ]
}

@test "a clone of an empty repository says it starts on the root commit" {
  [ "$(jf -R owner op log -n 1 --no-graph -T description)" = "check out the root commit" ]
}

@test "a fetch after an op restore catches up with the forge" {
  copy acme_owner jf -R owner commit -m "first"
  copy acme_owner jf -R owner bookmark create main -r @-
  copy acme_owner jf -R owner push --bookmark main
  copy acme_member jf -R member fetch
  before=$(jf -R member op log -n 1 --no-graph -T 'id')
  copy acme_owner jf -R owner commit -m "second"
  copy acme_owner jf -R owner bookmark set main -r @-
  copy acme_owner jf -R owner push --bookmark main
  copy acme_member jf -R member fetch

  copy acme_member jf -R member op restore "$before"
  copy acme_member jf -R member fetch
  [ "$(jf -R member log -r main --no-graph -T description)" = "second" ]
  copy acme_member jf -R member push --bookmark main
  [ "$(jf -R owner log -r main --no-graph -T description)" = "second" ]
}

@test "an undo of a fetch takes the clone back, and the next fetch brings it again" {
  copy acme_owner jf -R owner commit -m "first"
  copy acme_owner jf -R owner bookmark create main -r @-
  copy acme_owner jf -R owner push --bookmark main
  copy acme_member jf -R member fetch
  [ "$(jf -R member log -r main --no-graph -T description)" = "first" ]

  copy acme_member jf -R member undo
  [ -z "$(jf -R member bookmark list --all -T name)" ]

  copy acme_member jf -R member fetch
  [ "$(jf -R member log -r main --no-graph -T description)" = "first" ]
}

@test "an undo of a push keeps the push on the forge" {
  copy acme_owner jf -R owner commit -m "first"
  copy acme_owner jf -R owner bookmark create main -r @-
  copy acme_owner jf -R owner push --bookmark main

  copy acme_owner jf -R owner undo
  copy acme_member jf -R member fetch
  [ "$(jf -R member log -r main --no-graph -T description)" = "first" ]
  run copy acme_owner jf -R owner push --bookmark main
  [ "$status" -eq 0 ]
  [ "$(jf -R owner log -r main@forge --no-graph -T description)" = "first" ]
}
