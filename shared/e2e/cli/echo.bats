#!/usr/bin/env bats
# `jf echo` against a running forge that JJFORGE_ENDPOINT names. Exit codes
# and streams follow shared/docs/cli.md.

# `run --separate-stderr` needs Bats 1.5.
bats_require_minimum_version 1.5.0

@test "echo prints the message the forge answers, and its trace on stderr" {
  run --separate-stderr jf echo hello
  [ "$status" -eq 0 ]
  [ "$output" = "hello" ]
  [[ $stderr =~ ^trace\ id:\ [0-9a-f]{32}$ ]]
}

@test "echo joins the words with spaces" {
  run --separate-stderr jf echo hello there
  [ "$status" -eq 0 ]
  [ "$output" = "hello there" ]
}

@test "--endpoint overrides JJFORGE_ENDPOINT" {
  local forge=$JJFORGE_ENDPOINT
  JJFORGE_ENDPOINT=http://127.0.0.1:1 run --separate-stderr jf --endpoint "$forge" echo hi
  [ "$status" -eq 0 ]
  [ "$output" = "hi" ]
}

@test "echo without a message is bad usage" {
  run jf echo
  [ "$status" -eq 2 ]
}

@test "an endpoint that isn't an address is bad usage" {
  run jf --endpoint "not an address" echo hi
  [ "$status" -eq 2 ]
}

@test "an unreachable forge fails on stderr only" {
  run --separate-stderr jf --endpoint http://127.0.0.1:1 echo hi
  [ "$status" -eq 1 ]
  [ -z "$output" ]
  [ -n "$stderr" ]
}
