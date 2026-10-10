# The server serves the web app at /.
GET {{server}}/
HTTP 200
[Asserts]
body contains "id=\"root\""

# POST /api/v1/echo answers with the message, by way of vcs. Like every
# response, it names the trace of the request.
POST {{server}}/api/v1/echo
{
  "message": "hello"
}
HTTP 200
[Asserts]
jsonpath "$.message" == "hello"
header "traceresponse" matches /^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$/

# A long message comes back whole, with no cap.
POST {{server}}/api/v1/echo
{
  "message": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}
HTTP 200
[Asserts]
jsonpath "$.message" == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
