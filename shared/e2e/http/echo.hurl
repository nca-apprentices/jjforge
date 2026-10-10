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

# A 79-character message comes back whole, since the server caps no message.
POST {{server}}/api/v1/echo
{
  "message": "Long echo message passes through vcs whole, with no cap on how much it carries."
}
HTTP 200
[Asserts]
jsonpath "$.message" == "Long echo message passes through vcs whole, with no cap on how much it carries."
