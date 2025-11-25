# MinIO HTTPS Setup Guide

## Overview

This guide explains how to setup MinIO with HTTPS/TLS for the Vietnam Enterprise Cron System.

## Why HTTPS?

- **Production Ready**: HTTPS is required for production deployments
- **Security**: Encrypts data in transit
- **Compatibility**: rust-s3 crate works best with HTTPS
- **Best Practice**: Industry standard for object storage

## Setup Steps

### 1. Generate Certificates (Already Done)

Certificates have been generated in `config/certs/minio/`:
- `private.key` - Private key
- `public.crt` - Public certificate (self-signed, valid for 365 days)

To regenerate:
```bash
./scripts/generate-minio-certs.sh
```

### 2. Docker Compose Configuration

MinIO is configured to use HTTPS:
- Certificates mounted to `/root/.minio/certs/` in container
- Endpoint: `https://minio:9000`
- Health check uses HTTPS with `-k` flag (ignore self-signed cert)

### 3. Application Configuration

All services (API, Worker, Scheduler) use:
```
APP__MINIO__ENDPOINT=https://minio:9000
```

### 4. Rebuild and Restart

```bash
# Rebuild Docker images
docker-compose build

# Restart services
docker-compose up -d
```

## Development vs Production

### Development (Current Setup)
- Self-signed certificates
- Certificate validation may be relaxed
- Good for local testing

### Production (Recommended)
- Use proper CA-signed certificates (Let's Encrypt, DigiCert, etc.)
- Enable strict certificate validation
- Use proper domain names (not localhost/IP)

## Troubleshooting

### Issue: Certificate verification failed

**Solution 1**: Trust the certificate (macOS)
```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain config/certs/minio/public.crt
```

**Solution 2**: Use proper CA-signed certificate in production

### Issue: Connection refused

Check if MinIO is running with HTTPS:
```bash
docker logs vietnam-cron-minio
curl -k https://localhost:9000/minio/health/live
```

### Issue: rust-s3 SSL errors

The rust-s3 crate with `tokio-rustls-tls` feature handles HTTPS automatically. Self-signed certificates work because rustls accepts them by default in development.

## Files Modified

1. `Cargo.toml` - Reverted to rust-s3
2. `common/src/storage/minio.rs` - Updated MinIO client
3. `docker-compose.yml` - Added HTTPS configuration
4. `scripts/generate-minio-certs.sh` - Certificate generation script
5. `config/certs/minio/` - Certificate storage

## Next Steps

1. Rebuild Docker containers
2. Test job import functionality
3. For production: Replace self-signed certs with CA-signed certs

## Security Notes

- Self-signed certificates are for development only
- Private key should never be committed to git (add to .gitignore)
- Rotate certificates before expiry (365 days)
- Use proper secrets management in production
