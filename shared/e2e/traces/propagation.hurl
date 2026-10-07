# A request that carries traceparent continues its trace, names it in the
# answer, and leaves spans of the server and vcs under it, as ADR 0005
# decides. The binaries send spans in batches, so the second request asks
# again until both arrive.
POST {{server}}/api/v1/echo
traceparent: 00-{{trace}}-00f067aa0ba902b7-01
{
  "message": "traced"
}
HTTP 200
[Asserts]
header "traceresponse" startsWith "00-{{trace}}-"

GET {{traces}}/select/jaeger/api/traces/{{trace}}
[Options]
retry: 30
retry-interval: 1000ms
HTTP 200
[Asserts]
jsonpath "$.data[0].processes.*.serviceName" contains "jjforge-server"
jsonpath "$.data[0].processes.*.serviceName" contains "jjforge-vcs"
