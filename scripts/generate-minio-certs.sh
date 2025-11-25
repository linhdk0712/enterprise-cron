#!/bin/bash
# Generate self-signed certificates for MinIO
# This script creates certificates for local development

set -e

CERT_DIR="./config/certs/minio"
mkdir -p "$CERT_DIR"

echo "Generating self-signed certificate for MinIO..."

# Generate private key
openssl genrsa -out "$CERT_DIR/private.key" 2048

# Generate certificate signing request
openssl req -new -key "$CERT_DIR/private.key" -out "$CERT_DIR/cert.csr" -subj "/C=VN/ST=Hanoi/L=Hanoi/O=Vietnam Enterprise/OU=IT/CN=minio"

# Generate self-signed certificate (valid for 365 days)
openssl x509 -req -days 365 -in "$CERT_DIR/cert.csr" -signkey "$CERT_DIR/private.key" -out "$CERT_DIR/public.crt"

# Set proper permissions
chmod 644 "$CERT_DIR/public.crt"
chmod 600 "$CERT_DIR/private.key"

echo "Certificates generated successfully in $CERT_DIR"
echo "- Private key: $CERT_DIR/private.key"
echo "- Public certificate: $CERT_DIR/public.crt"
echo ""
echo "To use with MinIO, mount these files to /root/.minio/certs/ in the container"
