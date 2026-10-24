# pending
# A person copies the full history of a jj repository they already have into
# a forge repository, and repeats the copy to bring in what changed since.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo import)
  jf debug init-simple local
  as acme_owner jf -R local describe -m "first"
  as acme_owner jf -R local new -m "second"
  as acme_owner jf -R local bookmark create main -r @
}

@test "88.2 an import keeps every change ID" {
  run as acme_owner jf import local "$url"
  [ "$status" -eq 0 ]

  as acme_member jf clone "$url" member
  [ "$(jf -R member log -r '::main' --no-graph -T 'change_id ++ "\n"')" = \
    "$(jf -R local log -r '::main' --no-graph -T 'change_id ++ "\n"')" ]
}

@test "88.5 a second import brings in what changed since" {
  as acme_owner jf import local "$url"
  as acme_owner jf -R local new -m "third"
  as acme_owner jf -R local bookmark set main -r @

  run as acme_owner jf import local "$url"
  [ "$status" -eq 0 ]

  as acme_member jf clone "$url" member
  [ "$(jf -R member log -r main --no-graph -T description)" = "third" ]
}
