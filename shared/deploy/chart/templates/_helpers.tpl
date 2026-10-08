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

{{/*
The connection to Postgres, from the app secret that CloudNativePG keeps for
the database. The URL leaves out the password, so a log line that names the
URL never shows it.
*/}}
{{- define "jjforge.database" -}}
{{- $secret := required "server.database.secretName is required" .Values.server.database.secretName -}}
- name: DATABASE_HOST
  valueFrom:
    secretKeyRef: { name: {{ $secret }}, key: host }
- name: DATABASE_PORT
  valueFrom:
    secretKeyRef: { name: {{ $secret }}, key: port }
- name: DATABASE_NAME
  valueFrom:
    secretKeyRef: { name: {{ $secret }}, key: dbname }
- name: SPRING_DATASOURCE_URL
  value: jdbc:postgresql://$(DATABASE_HOST):$(DATABASE_PORT)/$(DATABASE_NAME)
- name: SPRING_DATASOURCE_USERNAME
  valueFrom:
    secretKeyRef: { name: {{ $secret }}, key: username }
- name: SPRING_DATASOURCE_PASSWORD
  valueFrom:
    secretKeyRef: { name: {{ $secret }}, key: password }
{{- end -}}
