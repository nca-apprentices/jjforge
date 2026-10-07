{{- define "jjforge.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "jjforge.fullname" -}}
{{- printf "%s-%s" .Release.Name (include "jjforge.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/* The OpenTelemetry settings that both binaries read, as ADR 0005 decides. */}}
{{- define "jjforge.telemetryEnv" -}}
{{- with .Values.telemetry.tracesEndpoint -}}
- name: OTEL_EXPORTER_OTLP_TRACES_ENDPOINT
  value: {{ . | quote }}
{{ end -}}
{{- with .Values.telemetry.environment -}}
- name: OTEL_RESOURCE_ATTRIBUTES
  value: deployment.environment.name={{ . }}
{{ end -}}
{{- end -}}

{{/*
The server image's tag, such as v0.2.4 for a release or the commit a preview
runs, so pods, Headlamp, and every log line name what they run.
*/}}
{{- define "jjforge.labels" -}}
app.kubernetes.io/name: {{ include "jjforge.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Values.server.image.tag | default .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}
