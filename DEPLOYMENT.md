# Vietnam Enterprise Cron System - Deployment Guide

This document provides comprehensive deployment instructions for the Vietnam Enterprise Cron System.

## Overview

The system provides three deployment options:
1. **Docker** - Single-host deployment using Docker Compose
2. **Kubernetes** - Production-grade deployment using Helm charts
3. **Manual** - Direct binary deployment (for development)

## Docker Deployment

### Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- 4GB RAM minimum
- 20GB disk space

### Quick Start

```bash
# Build the Docker image
docker build -t vietnam-cron:latest .

# Start all services
docker-compose up -d

# Check service status
docker-compose ps

# View logs
docker-compose logs -f api

# Access the dashboard
open http://localhost:8080
```

### Services

The Docker Compose setup includes:

- **PostgreSQL** (port 5432) - System database
- **Redis** (port 6379) - Distributed locking and rate limiting
- **NATS** (port 4222) - Job queue with JetStream
- **MinIO** (port 9000/9001) - Object storage for job definitions
- **Scheduler** - Job scheduling component (3 replicas)
- **Worker** - Job execution component (2 replicas)
- **API** - REST API and dashboard (port 8080)
- **Prometheus** (port 9091) - Metrics collection (optional, use `--profile monitoring`)
- **Grafana** (port 3000) - Metrics visualization (optional, use `--profile monitoring`)

### Configuration

Environment variables can be set in `docker-compose.yml` or via `.env` file:

```bash
# Database
APP_DATABASE__URL=postgresql://cronuser:cronpass@postgres:5432/vietnam_cron

# Redis
APP_REDIS__URL=redis://:redispass@redis:6379

# NATS
APP_NATS__URL=nats://nats:4222

# MinIO
APP_MINIO__ENDPOINT=minio:9000
APP_MINIO__ACCESS_KEY=minioadmin
APP_MINIO__SECRET_KEY=minioadmin
APP_MINIO__BUCKET_NAME=vietnam-cron
APP_MINIO__USE_SSL=false

# Authentication
APP_AUTH__MODE=database
APP_AUTH__JWT_SECRET=change-this-secret-in-production
APP_AUTH__JWT_EXPIRY_HOURS=24

# Observability
APP_OBSERVABILITY__LOG_LEVEL=info
APP_OBSERVABILITY__METRICS_PORT=9090
```

### Monitoring (Optional)

To enable Prometheus and Grafana:

```bash
docker-compose --profile monitoring up -d
```

Access:
- Prometheus: http://localhost:9091
- Grafana: http://localhost:3000 (admin/admin)

## Kubernetes Deployment

### Prerequisites

- Kubernetes 1.20+
- Helm 3.8+
- kubectl configured
- Persistent Volume provisioner

### Installation

```bash
# Add Helm repository (when available)
helm repo add vietnam-cron https://charts.vietnam-enterprise.com
helm repo update

# Or install from local chart
cd charts/vietnam-enterprise-cron

# Install with default values
helm install my-cron . --namespace cron-system --create-namespace

# Install with custom values
helm install my-cron . -f custom-values.yaml --namespace cron-system --create-namespace
```

### Configuration

Create a `custom-values.yaml` file:

```yaml
# Image configuration
image:
  registry: your-registry.com
  repository: vietnam-cron
  tag: "1.0.0"

# Scheduler configuration
scheduler:
  replicaCount: 5
  resources:
    limits:
      cpu: 1000m
      memory: 512Mi

# Worker configuration
worker:
  replicaCount: 10
  autoscaling:
    enabled: true
    minReplicas: 5
    maxReplicas: 20
  resources:
    limits:
      cpu: 2000m
      memory: 1Gi

# API configuration
api:
  replicaCount: 3
  ingress:
    enabled: true
    className: nginx
    hosts:
      - host: cron.example.com
        paths:
          - path: /
            pathType: Prefix
    tls:
      - secretName: cron-tls
        hosts:
          - cron.example.com

# Authentication
auth:
  mode: keycloak
  keycloak:
    serverUrl: https://keycloak.example.com
    realm: enterprise
    clientId: cron-system

# External services (if not using bundled)
postgresql:
  enabled: false

externalPostgresql:
  host: postgres.example.com
  port: 5432
  username: cronuser
  password: secretpassword
  database: vietnam_cron
  sslMode: require
```

### Upgrading

```bash
helm upgrade my-cron vietnam-cron/vietnam-enterprise-cron -f custom-values.yaml
```

### Uninstalling

```bash
helm uninstall my-cron --namespace cron-system
```

## Architecture

### Multi-Stage Docker Build

The Dockerfile uses a multi-stage build to minimize image size:

**Stage 1: Builder**
- Base: `rust:1.75-alpine`
- Installs build dependencies
- Compiles all binaries with optimizations
- Strips debug symbols

**Stage 2: Runtime**
- Base: `alpine:3.19`
- Minimal runtime dependencies (ca-certificates, tzdata)
- Non-root user (cronuser)
- Final image size: **< 50MB**

### Component Separation

Each component runs as a separate process:

- **Scheduler** (`scheduler` binary) - Detects jobs due for execution
- **Worker** (`worker` binary) - Executes jobs from the queue
- **API** (`api` binary) - Serves REST API and dashboard

This allows:
- Independent scaling of each component
- Resource optimization
- Fault isolation

### High Availability

**Scheduler:**
- Multiple replicas (3+)
- Distributed locking via Redis
- Pod anti-affinity rules

**Worker:**
- Horizontal Pod Autoscaler
- Scales based on CPU/memory and queue depth
- Graceful shutdown handling

**API:**
- Multiple replicas (2+)
- Load balanced via Kubernetes Service
- Health checks and readiness probes

## Security

### Container Security

- Non-root user (UID 1000)
- Read-only root filesystem
- Dropped all capabilities
- No privilege escalation

### Network Security

- TLS for external communication
- Network policies (optional)
- Secrets management via Kubernetes Secrets

### Authentication

Two modes supported:

**Database Mode:**
- Users stored in PostgreSQL
- Bcrypt password hashing
- JWT tokens for API access

**Keycloak Mode:**
- External identity provider
- JWT token validation
- Role-based access control

## Monitoring

### Metrics

Prometheus metrics exposed on port 9090:

- `job_success_total` - Successful job executions
- `job_failed_total` - Failed job executions
- `job_duration_seconds` - Job execution duration
- `job_queue_size` - Current queue depth
- `scheduler_lock_acquisitions_total` - Lock acquisitions
- `worker_executions_active` - Active executions

### Logging

Structured JSON logs with trace context:

```json
{
  "timestamp": "2025-01-15T10:30:00Z",
  "level": "INFO",
  "message": "Job execution started",
  "job_id": "123e4567-e89b-12d3-a456-426614174000",
  "execution_id": "987fcdeb-51a2-43f7-8765-123456789abc",
  "trace_id": "abc123",
  "span_id": "def456"
}
```

### Health Checks

- **Liveness probe**: `/health` endpoint
- **Readiness probe**: `/health` endpoint
- **Startup probe**: 30s initial delay

## Troubleshooting

### Pods Not Starting

```bash
# Check pod status
kubectl get pods -n cron-system

# View pod logs
kubectl logs -n cron-system deployment/my-cron-api

# Describe pod for events
kubectl describe pod -n cron-system <pod-name>
```

### Database Connection Issues

```bash
# Test PostgreSQL connectivity
kubectl run -it --rm debug --image=postgres:16 --restart=Never -- \
  psql -h my-cron-postgresql -U cronuser -d vietnam_cron

# Check database logs
kubectl logs -n cron-system statefulset/my-cron-postgresql
```

### Redis Connection Issues

```bash
# Test Redis connectivity
kubectl run -it --rm debug --image=redis:7 --restart=Never -- \
  redis-cli -h my-cron-redis-master -a redispass ping

# Check Redis logs
kubectl logs -n cron-system statefulset/my-cron-redis-master
```

### Migration Issues

```bash
# Check if migrations ran
kubectl logs -n cron-system deployment/my-cron-api -c run-migrations

# Manually run migrations
kubectl exec -it -n cron-system deployment/my-cron-api -- \
  sqlx migrate run --database-url $DATABASE_URL
```

## Performance Tuning

### Worker Scaling

Adjust based on workload:

```yaml
worker:
  autoscaling:
    enabled: true
    minReplicas: 5
    maxReplicas: 50
    targetCPUUtilizationPercentage: 70
```

### Database Connection Pool

```yaml
extraEnvVars:
  - name: APP_DATABASE__MAX_CONNECTIONS
    value: "50"
  - name: APP_DATABASE__MIN_CONNECTIONS
    value: "10"
```

### Redis Pool Size

```yaml
extraEnvVars:
  - name: APP_REDIS__POOL_SIZE
    value: "20"
```

## Backup and Recovery

### Database Backup

```bash
# Backup PostgreSQL
kubectl exec -n cron-system statefulset/my-cron-postgresql -- \
  pg_dump -U cronuser vietnam_cron > backup.sql

# Restore PostgreSQL
kubectl exec -i -n cron-system statefulset/my-cron-postgresql -- \
  psql -U cronuser vietnam_cron < backup.sql
```

### MinIO Backup

```bash
# Backup MinIO data
kubectl exec -n cron-system statefulset/my-cron-minio -- \
  mc mirror /data /backup
```

## Production Checklist

- [ ] Change default passwords
- [ ] Configure TLS/SSL
- [ ] Set up monitoring and alerting
- [ ] Configure backup strategy
- [ ] Set resource limits appropriately
- [ ] Enable Pod Disruption Budgets
- [ ] Configure Network Policies
- [ ] Set up log aggregation
- [ ] Configure persistent storage
- [ ] Test disaster recovery procedures

## Support

For issues and questions:
- GitHub Issues: https://github.com/vietnam-enterprise/cron-system/issues
- Documentation: https://github.com/vietnam-enterprise/cron-system
