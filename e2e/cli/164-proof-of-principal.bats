# pending
# A name proves nothing. vcs serves a request only when its token carries
# the server's signature, so a deployed forge serves the people who signed
# in and refuses everyone else.

load forge

setup() {
  cd "$BATS_TEST_TMPDIR"
  url=$(vcs_repo)
  token=$(sign_in acme-owner acme)
}

# Prints the status and the problem code of `GET {url}/head` with the
# Authorization header $1, such as `401 unauthenticated`.
head_with() {
  local args=(-sS -o body -w '%{http_code}')
  [[ -n $1 ]] && args+=(-H "Authorization: $1")
  local status
  status=$(curl "${args[@]}" "$url/head")
  echo "$status $(jq -r '.code // empty' body 2>/dev/null)"
}

# Encodes stdin as unpadded base64url, the encoding of each part of a JWT.
b64url() {
  openssl base64 -A | tr '+/' '-_' | tr -d '='
}

# Decodes the base64url part $1 of a JWT.
unb64url() {
  local part=$1
  while ((${#part} % 4)); do part+="="; done
  printf '%s' "$part" | tr '_-' '/+' | openssl base64 -d -A
}

@test "164.1 a token that the server signed reaches the repository" {
  run head_with "Bearer $token"
  [ "$output" = "200 " ]
}

@test "164.2 a bare name gets 401" {
  run head_with "Bearer acme-owner"
  [ "$output" = "401 unauthenticated" ]
}

@test "164.3 a request without a token gets 401" {
  run head_with ""
  [ "$output" = "401 unauthenticated" ]
}

@test "164.4 a token whose claims changed after signing gets 401" {
  local header claims signature forged
  IFS=. read -r header claims signature <<<"$token"
  forged=$(unb64url "$claims" | jq -c '.name = "other-owner"' | b64url)
  run head_with "Bearer $header.$forged.$signature"
  [ "$output" = "401 unauthenticated" ]
}

@test "164.5 a token with the algorithm none gets 401" {
  local header claims
  header=$(printf '{"alg":"none","typ":"at+jwt"}' | b64url)
  claims=$(printf '{"iss":"%s","aud":"jjforge","sub":"x","name":"acme-owner","exp":%d}' \
    "$JJFORGE_ENDPOINT" "$(($(date +%s) + 600))" | b64url)
  run head_with "Bearer $header.$claims."
  [ "$output" = "401 unauthenticated" ]
}

@test "164.6 a token signed by a key the server never published gets 401" {
  local header claims signature
  header=$(printf '{"alg":"EdDSA","typ":"at+jwt","kid":"2026-11"}' | b64url)
  claims=$(printf '{"iss":"%s","aud":"jjforge","sub":"x","name":"acme-owner","exp":%d}' \
    "$JJFORGE_ENDPOINT" "$(($(date +%s) + 600))" | b64url)
  openssl genpkey -algorithm ed25519 -out own.pem
  signature=$(printf '%s.%s' "$header" "$claims" |
    openssl pkeyutl -sign -inkey own.pem -rawin -in /dev/stdin | b64url)
  run head_with "Bearer $header.$claims.$signature"
  [ "$output" = "401 unauthenticated" ]
}

@test "164.7 jf with a bare name in JJFORGE_TOKEN exits with 5" {
  run env JJFORGE_TOKEN=acme-owner jf clone "$url" denied
  [ "$status" -eq 5 ]
  [ ! -e denied ]
}

@test "164.8 the gRPC services refuse a bare name too" {
  # shellcheck disable=SC2154 # forge.bash sets acme_org.
  run grpcurl -plaintext -import-path "$BATS_TEST_DIRNAME/../../contracts/proto" \
    -proto source/v1/repo.proto -H "authorization: Bearer acme-owner" \
    -d "{\"repo\": {\"org_id\": \"$acme_org\", \"repo_id\": \"$(uuidgen)\"}}" \
    "localhost:${JJFORGE_VCS_PORT:-50052}" source.v1.RepoService/CreateRepo
  [ "$status" -ne 0 ]
  [[ $output == *Unauthenticated* ]]
}
