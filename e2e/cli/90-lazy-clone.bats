# pending
# A person starts working in a fresh clone without waiting for files they
# don't open.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(new_repo large)
  as acme_owner jf clone "$url" owner
  mkdir owner/docs owner/src
  for n in $(seq 1 1000); do
    echo "doc $n" >"owner/docs/$n.md"
    echo "src $n" >"owner/src/$n.rs"
  done
  as acme_owner jf -R owner describe -m "many files"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
}

@test "90.4 history works in a fresh clone with the forge gone" {
  as acme_member jf clone "$url" member

  JJFORGE_ENDPOINT=http://127.0.0.1:1 run jf -R member log -r main --no-graph -T description
  [ "$status" -eq 0 ]
  [ "$output" = "many files" ]
}
