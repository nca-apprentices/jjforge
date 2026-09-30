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
