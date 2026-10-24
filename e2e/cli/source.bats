#!/usr/bin/env bats
# The server reads a pushed repository through source/v1, as #159 and #160
# ask: its log, a file, a diff, and each push as it lands.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(vcs_repo)
  repo=${url##*/}
  copy acme_owner jf clone "$url" owner
  echo hello >owner/README
  copy acme_owner jf -R owner commit -m "first"
  copy acme_owner jf -R owner bookmark create main -r @-
  copy acme_owner jf -R owner push --bookmark main
}

# Calls a method of source/v1 as the owner, on the repository of the test,
# with more fields and then grpcurl flags, such as
# `call FileService/ReadFile '"rev": "main", "path": "README"'`.
# shellcheck disable=SC2154 # forge.bash sets acme_org.
call() {
  local method=$1 fields=${2:-}
  shift
  [[ $# -eq 0 ]] || shift
  grpcurl -plaintext -import-path "$BATS_TEST_DIRNAME/../../contracts/proto" \
    -proto source/v1/history.proto -proto source/v1/file.proto \
    -proto source/v1/diff.proto -proto source/v1/repo.proto \
    -H "authorization: Bearer acme_owner" \
    -d "{\"repo\": {\"org_id\": \"$acme_org\", \"repo_id\": \"$repo\"}${fields:+, $fields}}" \
    "$@" "localhost:${JJFORGE_VCS_PORT:-50052}" "source.v1.$method"
}

@test "the log shows the pushed changes from trunk, newest first" {
  echo world >>owner/README
  copy acme_owner jf -R owner commit -m "second"
  copy acme_owner jf -R owner bookmark set main -r @-
  copy acme_owner jf -R owner push --bookmark main

  run call HistoryService/Log
  [ "$status" -eq 0 ]
  [ "$(jq -rs '.[0].change.description' <<<"$output")" = "second" ]
  [ "$(jq -rs '.[1].change.description' <<<"$output")" = "first" ]
}

@test "a file reads at a bookmark with its size" {
  run call FileService/ReadFile '"rev": "main", "path": "README"'
  [ "$status" -eq 0 ]
  [ "$(jq -r '.size' <<<"$output")" = "6" ]
  [ "$(jq -r '.chunk' <<<"$output" | base64 --decode)" = "hello" ]
}

@test "the diff of a change names each file it added" {
  run call DiffService/Diff '"rev": "main"'
  [ "$status" -eq 0 ]
  [ "$(jq -r '.path + " " + .status' <<<"$output")" = "README STATUS_ADDED" ]
}

# grpcurl keeps gRPC's default limit of 4 MiB per message, as the server's
# client does, so the diff of a 2.7 MB file must arrive truncated.
@test "the diff of a large text file reaches a client with the default limit" {
  seq 1 400000 >owner/numbers
  copy acme_owner jf -R owner --config snapshot.max-new-file-size=10000000 commit -m "numbers"
  copy acme_owner jf -R owner bookmark set main -r @-
  copy acme_owner jf -R owner push --bookmark main

  run call DiffService/Diff '"rev": "main"'
  [ "$status" -eq 0 ]
  [ "$(jq -r '.path + " " + (.truncated | tostring)' <<<"$output")" = "numbers true" ]
}

@test "a watch yields the push that lands after the operation it names" {
  head=$(call RepoService/GetHead | jq -r .operationId)
  call HistoryService/WatchOperations "\"since\": \"$head\"" -max-time 5 >watch.json || true &
  sleep 1
  echo more >>owner/README
  copy acme_owner jf -R owner commit -m "third"
  copy acme_owner jf -R owner bookmark set main -r @-
  copy acme_owner jf -R owner push --bookmark main
  wait

  [ "$(jq -rs 'length' <watch.json)" = "1" ]
  [ "$(jq -rs '.[0].operation.parentIds[0]' <watch.json)" = "$head" ]
}

@test "a change lists its first files, and ListChangedFiles the rest" {
  for n in $(seq -w 1 150); do echo "$n" >"owner/f$n"; done
  copy acme_owner jf -R owner commit -m "many"
  copy acme_owner jf -R owner bookmark set main -r @-
  copy acme_owner jf -R owner push --bookmark main

  first=$(call HistoryService/GetChange '"rev": "main"')
  [ "$(jq '.files | length' <<<"$first")" = "100" ]
  token=$(jq -r .filesNextPageToken <<<"$first")
  run call DiffService/ListChangedFiles "\"rev\": \"main\", \"pageToken\": \"$token\""
  [ "$status" -eq 0 ]
  [ "$(jq '.files | length' <<<"$output")" = "50" ]
  [ "$(jq -r '.files[0].path' <<<"$output")" = "f101" ]
  [ "$(jq -r '.nextPageToken' <<<"$output")" = "null" ]
}
