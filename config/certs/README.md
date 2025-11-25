# TLS Certificates for MinIO

## Generate Certificates

Run the script to generate self-signed certificates for local development:

```bash
./scripts/generate-minio-certs.sh
```

This will create:
- `minio/private.key` - Private key for MinIO
- `minio/public.crt` - Public certificate for MinIO

## Production Setup

For production, use proper certificates from a Certificate Authority (CA) like:
- Let's Encrypt (free)
- DigiCert
- Comodo

## Docker Setup

The certificates are mounted to MinIO container at `/root/.minio/certs/`:
- `/root/.minio/certs/private.key`
- `/root/.minio/certs/public.crt`

MinIO will automatically use these certificates for HTTPS.

## Trust Certificate (Development Only)

For development, you may need to trust the self-signed certificate or disable SSL verification in your application.

### Option 1: Trust the certificate (macOS)
```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain config/certs/minio/public.crt
```

### Option 2: Disable SSL verification (Development only - NOT for production)
This is handled in the application code with proper error handling.
