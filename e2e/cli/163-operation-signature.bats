# pending
# vcs signs every operation it writes, and anyone with a clone checks the
# signatures against the key that the forge publishes.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(vcs_repo)
  as acme_owner jf clone "$url" owner
  as acme_owner jf -R owner describe -m "signed"
  as acme_owner jf -R owner bookmark create main -r @
  as acme_owner jf -R owner push --bookmark main
}

@test "163.2 the forge publishes its key without a token" {
  run curl -sS -o /dev/null -w '%{http_code} %{content_type}' \
    "${url%%/sync/v1/*}/sync/v1/keys"
  [ "$output" = "200 application/x-protobuf" ]
}

@test "163.1 the operation of a push carries the forge's signature" {
  run jf -R owner op verify
  [ "$status" -eq 0 ]
  [[ $output == *"signed by the forge"* ]]
}

@test "163.3 jf op verify checks every operation the forge wrote" {
  as acme_member jf clone "$url" member
  run jf -R member op verify
  [ "$status" -eq 0 ]
  [[ $output == *"signed by the forge"* ]]
  [[ $output != *unsigned* ]]
}
