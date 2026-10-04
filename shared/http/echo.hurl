# The server serves the web app at /.
GET {{server}}/
HTTP 200
[Asserts]
body contains "id=\"root\""

# POST /api/v1/echo answers with the message, by way of vcs.
POST {{server}}/api/v1/echo
{
  "message": "hello"
}
HTTP 200
[Asserts]
jsonpath "$.message" == "hello"
