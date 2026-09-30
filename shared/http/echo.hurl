# The server serves the web app at /.
GET {{server}}/
HTTP 200
[Asserts]
body contains "id=\"root\""

# POST /api/echo answers with the message, by way of vcsd.
POST {{server}}/api/echo
{
  "message": "hello"
}
HTTP 200
[Asserts]
jsonpath "$.message" == "hello"
