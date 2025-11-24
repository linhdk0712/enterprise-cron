{{/*
Expand the name of the chart.
*/}}
{{- define "vietnam-cron.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "vietnam-cron.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "vietnam-cron.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "vietnam-cron.labels" -}}
helm.sh/chart: {{ include "vietnam-cron.chart" . }}
{{ include "vietnam-cron.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "vietnam-cron.selectorLabels" -}}
app.kubernetes.io/name: {{ include "vietnam-cron.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "vietnam-cron.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "vietnam-cron.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
PostgreSQL host
*/}}
{{- define "vietnam-cron.postgresql.host" -}}
{{- if .Values.postgresql.enabled }}
{{- printf "%s-postgresql" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- .Values.externalPostgresql.host }}
{{- end }}
{{- end }}

{{/*
PostgreSQL port
*/}}
{{- define "vietnam-cron.postgresql.port" -}}
{{- if .Values.postgresql.enabled }}
{{- 5432 }}
{{- else }}
{{- .Values.externalPostgresql.port }}
{{- end }}
{{- end }}

{{/*
PostgreSQL database
*/}}
{{- define "vietnam-cron.postgresql.database" -}}
{{- if .Values.postgresql.enabled }}
{{- .Values.postgresql.auth.database }}
{{- else }}
{{- .Values.externalPostgresql.database }}
{{- end }}
{{- end }}

{{/*
PostgreSQL username
*/}}
{{- define "vietnam-cron.postgresql.username" -}}
{{- if .Values.postgresql.enabled }}
{{- .Values.postgresql.auth.username }}
{{- else }}
{{- .Values.externalPostgresql.username }}
{{- end }}
{{- end }}

{{/*
PostgreSQL password secret name
*/}}
{{- define "vietnam-cron.postgresql.secretName" -}}
{{- if .Values.postgresql.enabled }}
{{- printf "%s-postgresql" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- printf "%s-external-postgresql" (include "vietnam-cron.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Redis host
*/}}
{{- define "vietnam-cron.redis.host" -}}
{{- if .Values.redis.enabled }}
{{- printf "%s-redis-master" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- .Values.externalRedis.host }}
{{- end }}
{{- end }}

{{/*
Redis port
*/}}
{{- define "vietnam-cron.redis.port" -}}
{{- if .Values.redis.enabled }}
{{- 6379 }}
{{- else }}
{{- .Values.externalRedis.port }}
{{- end }}
{{- end }}

{{/*
Redis password secret name
*/}}
{{- define "vietnam-cron.redis.secretName" -}}
{{- if .Values.redis.enabled }}
{{- printf "%s-redis" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- printf "%s-external-redis" (include "vietnam-cron.fullname" .) }}
{{- end }}
{{- end }}

{{/*
NATS URL
*/}}
{{- define "vietnam-cron.nats.url" -}}
{{- if .Values.nats.enabled }}
{{- printf "nats://%s-nats:4222" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- .Values.externalNats.url }}
{{- end }}
{{- end }}

{{/*
MinIO endpoint
*/}}
{{- define "vietnam-cron.minio.endpoint" -}}
{{- if .Values.minio.enabled }}
{{- printf "%s-minio:9000" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- .Values.externalMinio.endpoint }}
{{- end }}
{{- end }}

{{/*
MinIO secret name
*/}}
{{- define "vietnam-cron.minio.secretName" -}}
{{- if .Values.minio.enabled }}
{{- printf "%s-minio" (include "vietnam-cron.fullname" .) }}
{{- else }}
{{- printf "%s-external-minio" (include "vietnam-cron.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Image pull secrets
*/}}
{{- define "vietnam-cron.imagePullSecrets" -}}
{{- if .Values.global.imagePullSecrets }}
{{- range .Values.global.imagePullSecrets }}
- name: {{ . }}
{{- end }}
{{- else if .Values.image.pullSecrets }}
{{- range .Values.image.pullSecrets }}
- name: {{ . }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Return the proper image name
*/}}
{{- define "vietnam-cron.image" -}}
{{- $registryName := .Values.image.registry -}}
{{- $repositoryName := .Values.image.repository -}}
{{- $tag := .Values.image.tag | toString -}}
{{- if .Values.global.imageRegistry }}
    {{- $registryName = .Values.global.imageRegistry -}}
{{- end -}}
{{- printf "%s/%s:%s" $registryName $repositoryName $tag -}}
{{- end }}
