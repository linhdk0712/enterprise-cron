# Vietnam Enterprise Cron System - Helm Chart

A Helm chart for deploying the Vietnam Enterprise Cron System on Kubernetes.

## Prerequisites

- Kubernetes 1.20+
- Helm 3.8+
- PV provisioner support in the underlying infrastructure (for persistent storage)

## Installing the Chart

To install the chart with the release name `my-cron`:

```bash
helm repo add vietnam-cron https://charts.vietnam-enterprise.com
helm install my-cron vietnam-cron/vietnam-enterprise-cron
```

Or install from local chart:

```bash
helm install my-cron ./charts/vietnam-enterprise-cron
```

## Uninstalling the Chart

To uninstall/delete the `my-cron` deployment:

```bash
helm delete my-cron
```

## Configuration

The following table lists the configurable parameters of the Vietnam Enterprise Cron chart and their default values.

### Global Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `global.imageRegistry` | Global Docker image registry | `""` |
| `global.imagePullSecrets` | Global Docker registry secret names as an array | `[]` |
| `global.storageClass` | Global storage class for persistent volumes | `""` |

### Image Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `image.registry` | Image registry | `docker.io` |
| `image.repository` | Image repository | `vietnam-enterprise/cron-system` |
| `image.tag` | Image tag | `1.0.0` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |

### Scheduler Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `scheduler.enabled` | Enable scheduler component | `true` |
| `scheduler.replicaCount` | Number of scheduler replicas | `3` |
| `scheduler.resources.limits.cpu` | CPU limit | `500m` |
| `scheduler.resources.limits.memory` | Memory limit | `256Mi` |
| `scheduler.config.pollIntervalSeconds` | Job polling interval | `10` |
| `scheduler.config.lockTtlSeconds` | Distributed lock TTL | `30` |

### Worker Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `worker.enabled` | Enable worker component | `true` |
| `worker.replicaCount` | Number of worker replicas | `2` |
| `worker.resources.limits.cpu` | CPU limit | `1000m` |
| `worker.resources.limits.memory` | Memory limit | `512Mi` |
| `worker.config.concurrency` | Worker concurrency | `10` |
| `worker.config.maxRetries` | Maximum retry attempts | `10` |
| `worker.config.timeoutSeconds` | Job timeout | `300` |
| `worker.autoscaling.enabled` | Enable HPA for workers | `true` |
| `worker.autoscaling.minReplicas` | Minimum replicas | `2` |
| `worker.autoscaling.maxReplicas` | Maximum replicas | `10` |

### API Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `api.enabled` | Enable API component | `true` |
| `api.replicaCount` | Number of API replicas | `2` |
| `api.service.type` | Kubernetes service type | `ClusterIP` |
| `api.service.port` | API service port | `8080` |
| `api.ingress.enabled` | Enable ingress | `false` |
| `api.ingress.className` | Ingress class name | `nginx` |
| `api.ingress.hosts` | Ingress hosts configuration | `[]` |

### Authentication Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `auth.mode` | Authentication mode (database or keycloak) | `database` |
| `auth.jwtSecret` | JWT secret for database mode | `""` (auto-generated) |
| `auth.jwtExpiryHours` | JWT token expiry in hours | `24` |
| `auth.keycloak.serverUrl` | Keycloak server URL | `""` |
| `auth.keycloak.realm` | Keycloak realm | `""` |
| `auth.keycloak.clientId` | Keycloak client ID | `""` |

### PostgreSQL Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `postgresql.enabled` | Deploy PostgreSQL | `true` |
| `postgresql.auth.username` | PostgreSQL username | `cronuser` |
| `postgresql.auth.password` | PostgreSQL password | `cronpass` |
| `postgresql.auth.database` | PostgreSQL database | `vietnam_cron` |
| `postgresql.primary.persistence.size` | Primary PVC size | `10Gi` |

### Redis Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `redis.enabled` | Deploy Redis | `true` |
| `redis.architecture` | Redis architecture (standalone or replication) | `replication` |
| `redis.auth.password` | Redis password | `redispass` |
| `redis.master.persistence.size` | Master PVC size | `8Gi` |

### NATS Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `nats.enabled` | Deploy NATS | `true` |
| `nats.nats.jetstream.enabled` | Enable JetStream | `true` |
| `nats.cluster.enabled` | Enable NATS cluster | `true` |
| `nats.cluster.replicas` | Number of NATS replicas | `3` |

### MinIO Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `minio.enabled` | Deploy MinIO | `true` |
| `minio.mode` | MinIO mode (standalone or distributed) | `standalone` |
| `minio.rootUser` | MinIO root user | `minioadmin` |
| `minio.rootPassword` | MinIO root password | `minioadmin` |
| `minio.persistence.size` | MinIO PVC size | `50Gi` |

## Examples

### Using External PostgreSQL

```yaml
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

### Using Keycloak Authentication

```yaml
auth:
  mode: keycloak
  keycloak:
    serverUrl: https://keycloak.example.com
    realm: enterprise
    clientId: cron-system
```

### Enabling Ingress

```yaml
api:
  ingress:
    enabled: true
    className: nginx
    annotations:
      cert-manager.io/cluster-issuer: letsencrypt-prod
    hosts:
      - host: cron.example.com
        paths:
          - path: /
            pathType: Prefix
    tls:
      - secretName: cron-tls
        hosts:
          - cron.example.com
```

### High Availability Setup

```yaml
scheduler:
  replicaCount: 5
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        - labelSelector:
            matchExpressions:
              - key: app.kubernetes.io/component
                operator: In
                values:
                  - scheduler
          topologyKey: kubernetes.io/hostname

worker:
  replicaCount: 10
  autoscaling:
    enabled: true
    minReplicas: 5
    maxReplicas: 20

api:
  replicaCount: 3
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10

postgresql:
  readReplicas:
    replicaCount: 2

redis:
  replica:
    replicaCount: 3
```

## Upgrading

To upgrade the chart:

```bash
helm upgrade my-cron vietnam-cron/vietnam-enterprise-cron -f custom-values.yaml
```

## Troubleshooting

### Pods not starting

Check pod logs:
```bash
kubectl logs -l app.kubernetes.io/instance=my-cron
```

Check pod events:
```bash
kubectl describe pod -l app.kubernetes.io/instance=my-cron
```

### Database connection issues

Verify PostgreSQL is running:
```bash
kubectl get pods -l app.kubernetes.io/name=postgresql
```

Test database connectivity:
```bash
kubectl run -it --rm debug --image=postgres:16 --restart=Never -- \
  psql -h my-cron-postgresql -U cronuser -d vietnam_cron
```

### Redis connection issues

Verify Redis is running:
```bash
kubectl get pods -l app.kubernetes.io/name=redis
```

Test Redis connectivity:
```bash
kubectl run -it --rm debug --image=redis:7 --restart=Never -- \
  redis-cli -h my-cron-redis-master -a redispass ping
```

## Support

For issues and questions:
- GitHub Issues: https://github.com/vietnam-enterprise/cron-system/issues
- Documentation: https://github.com/vietnam-enterprise/cron-system/blob/main/README.md
