{{- define "jjforge.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "jjforge.fullname" -}}
{{- printf "%s-%s" .Release.Name (include "jjforge.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "jjforge.labels" -}}
app.kubernetes.io/name: {{ include "jjforge.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{/*
The connection to Postgres, from the app secret that CloudNativePG keeps for
the database. The URL leaves out the password, so a log line that names the
URL never shows it.
*/}}
{{- define "jjforge.database" -}}
- name: DATABASE_HOST
  valueFrom:
    secretKeyRef: { name: {{ .Values.server.database.secretName }}, key: host }
- name: DATABASE_PORT
  valueFrom:
    secretKeyRef: { name: {{ .Values.server.database.secretName }}, key: port }
- name: DATABASE_NAME
  valueFrom:
    secretKeyRef: { name: {{ .Values.server.database.secretName }}, key: dbname }
- name: SPRING_DATASOURCE_URL
  value: jdbc:postgresql://$(DATABASE_HOST):$(DATABASE_PORT)/$(DATABASE_NAME)
- name: SPRING_DATASOURCE_USERNAME
  valueFrom:
    secretKeyRef: { name: {{ .Values.server.database.secretName }}, key: username }
- name: SPRING_DATASOURCE_PASSWORD
  valueFrom:
    secretKeyRef: { name: {{ .Values.server.database.secretName }}, key: password }
{{- end -}}
