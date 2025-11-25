# Vietnam Enterprise Cron System - API Test Commands

Quick reference for testing the API with curl commands.

## 🔐 Authentication

### 1. Login (Admin)
```bash
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "admin123"
  }' \
  -c cookies.txt \
  -i
```

**Response:**
```json
{
  "data": {
    "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "expires_at": 1764119968
  }
}
```

**Headers:**
```
set-cookie: auth_token=eyJ0eXAi...; HttpOnly; SameSite=Lax; Path=/; Max-Age=86400
```

### 2. Save Token to Variable
```bash
# Extract token from response
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')

echo "Token: $TOKEN"
```

### 3. Refresh Token
```bash
curl -X POST http://localhost:8080/api/auth/refresh \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\"}"
```

---

## 📊 Dashboard (Browser/Cookie Auth)

### Access Dashboard with Cookie
```bash
curl -X GET http://localhost:8080/dashboard \
  -b cookies.txt
```

---

## 💼 Jobs API

### List All Jobs
```bash
curl -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN"
```

### Create HTTP Job
```bash
curl -X POST http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Fetch API Data",
    "description": "Get data from external API every 5 minutes",
    "job_type": "http",
    "schedule_type": "cron",
    "schedule_config": {
      "cron_expression": "0 */5 * * * *"
    },
    "job_config": {
      "url": "https://api.example.com/data",
      "method": "GET",
      "headers": {
        "Accept": "application/json"
      }
    },
    "enabled": true
  }'
```

### Get Job by ID
```bash
JOB_ID="00000000-0000-0000-0000-000000000000"
curl -X GET http://localhost:8080/api/jobs/$JOB_ID \
  -H "Authorization: Bearer $TOKEN"
```

### Update Job
```bash
curl -X PUT http://localhost:8080/api/jobs/$JOB_ID \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Updated Job Name",
    "description": "Updated description",
    "enabled": false
  }'
```

### Trigger Job Manually
```bash
curl -X POST http://localhost:8080/api/jobs/$JOB_ID/trigger \
  -H "Authorization: Bearer $TOKEN"
```

### Enable Job
```bash
curl -X PUT http://localhost:8080/api/jobs/$JOB_ID/enable \
  -H "Authorization: Bearer $TOKEN"
```

### Disable Job
```bash
curl -X PUT http://localhost:8080/api/jobs/$JOB_ID/disable \
  -H "Authorization: Bearer $TOKEN"
```

### Delete Job (Admin Only)
```bash
curl -X DELETE http://localhost:8080/api/jobs/$JOB_ID \
  -H "Authorization: Bearer $TOKEN"
```

---

## 📝 Executions

### List All Executions
```bash
curl -X GET http://localhost:8080/api/executions \
  -H "Authorization: Bearer $TOKEN"
```

### Get Execution by ID
```bash
EXEC_ID="00000000-0000-0000-0000-000000000000"
curl -X GET http://localhost:8080/api/executions/$EXEC_ID \
  -H "Authorization: Bearer $TOKEN"
```

---

## 👥 User Management (Admin Only)

### List Users
```bash
curl -X GET http://localhost:8080/api/users \
  -H "Authorization: Bearer $TOKEN"
```

### Create User
```bash
curl -X POST http://localhost:8080/api/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "newuser",
    "password": "SecurePassword123!",
    "email": "newuser@example.com"
  }'
```

### Get User
```bash
USER_ID="00000000-0000-0000-0000-000000000000"
curl -X GET http://localhost:8080/api/users/$USER_ID \
  -H "Authorization: Bearer $TOKEN"
```

### Assign Roles to User
```bash
# Admin role ID: 00000000-0000-0000-0000-000000000001
# Regular User role ID: 00000000-0000-0000-0000-000000000002

curl -X PUT http://localhost:8080/api/users/$USER_ID/roles \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "role_ids": ["00000000-0000-0000-0000-000000000002"]
  }'
```

### List All Roles
```bash
curl -X GET http://localhost:8080/api/roles \
  -H "Authorization: Bearer $TOKEN"
```

### Update User Password
```bash
curl -X PUT http://localhost:8080/api/users/$USER_ID/password \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "new_password": "NewSecurePassword456!"
  }'
```

### Delete User
```bash
curl -X DELETE http://localhost:8080/api/users/$USER_ID \
  -H "Authorization: Bearer $TOKEN"
```

---

## 🔧 Variables

### List Variables
```bash
curl -X GET http://localhost:8080/api/variables \
  -H "Authorization: Bearer $TOKEN"
```

### Create Variable (Admin Only)
```bash
curl -X POST http://localhost:8080/api/variables \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "API_KEY",
    "value": "secret-key-value",
    "description": "External API key",
    "is_encrypted": true
  }'
```

### Update Variable (Admin Only)
```bash
VAR_ID="00000000-0000-0000-0000-000000000000"
curl -X PUT http://localhost:8080/api/variables/$VAR_ID \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "value": "new-secret-value",
    "is_encrypted": true
  }'
```

### Delete Variable (Admin Only)
```bash
curl -X DELETE http://localhost:8080/api/variables/$VAR_ID \
  -H "Authorization: Bearer $TOKEN"
```

---

## ℹ️ System Info (Public)

### API Info
```bash
curl -X GET http://localhost:8080/api/info
```

### Health Check
```bash
curl -X GET http://localhost:8080/health
```

---

## 🧪 Complete Test Flow

```bash
#!/bin/bash

# 1. Login and save token
echo "=== LOGIN ==="
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')

echo "Token: ${TOKEN:0:50}..."
echo ""

# 2. List jobs
echo "=== LIST JOBS ==="
curl -s -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN" | jq '.'
echo ""

# 3. Create a test job
echo "=== CREATE JOB ==="
JOB_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Test Job",
    "description": "Test HTTP job",
    "job_type": "http",
    "schedule_type": "cron",
    "schedule_config": {"cron_expression": "0 */5 * * * *"},
    "job_config": {
      "url": "https://httpbin.org/get",
      "method": "GET"
    },
    "enabled": true
  }')

echo "$JOB_RESPONSE" | jq '.'
JOB_ID=$(echo "$JOB_RESPONSE" | jq -r '.data.id')
echo ""

# 4. Trigger the job
echo "=== TRIGGER JOB ==="
curl -s -X POST http://localhost:8080/api/jobs/$JOB_ID/trigger \
  -H "Authorization: Bearer $TOKEN" | jq '.'
echo ""

# 5. List executions
echo "=== LIST EXECUTIONS ==="
curl -s -X GET http://localhost:8080/api/executions \
  -H "Authorization: Bearer $TOKEN" | jq '.'
```

---

## 🔒 RBAC Permissions Reference

### Admin Role (18 permissions)
- `job:read` - View jobs
- `job:write` - Create/update jobs
- `job:execute` - Trigger jobs
- `job:delete` - Delete jobs
- `job:import` - Import jobs
- `job:export` - Export jobs
- `execution:read` - View all executions
- `execution:stop` - Stop running executions
- `variable:read` - View all variables
- `variable:write` - Create/update variables
- `variable:encrypt` - Encrypt sensitive values
- `webhook:read` - View webhooks
- `webhook:write` - Create/update webhooks
- `user:manage` - Manage users
- `role:assign` - Assign roles
- `system:config` - System configuration
- `system:audit` - Access audit logs
- `dashboard:admin` - Full dashboard access

### Regular User Role (5 permissions)
- `job:read` - View jobs (read-only)
- `job:execute` - Trigger jobs
- `execution:read` - View own executions
- `variable:read` - View non-sensitive variables
- `dashboard:user` - Limited dashboard access

---

## 📋 Tips

### Pretty Print JSON
```bash
curl ... | jq '.'
```

### Save Response to File
```bash
curl ... -o response.json
```

### Show Response Headers
```bash
curl ... -i
```

### Verbose Output (Debug)
```bash
curl ... -v
```

### Follow Redirects
```bash
curl ... -L
```

### Set Custom Timeout
```bash
curl ... --max-time 30
```

### Test with Invalid Token (Expect 401)
```bash
curl -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer invalid-token"
```

### Test Permission Denial (Expect 403)
```bash
# Login as regular user, try to delete job (requires job:delete)
# Will return 403 Forbidden
```

---

## 🐛 Debugging

### Check API is Running
```bash
curl http://localhost:8080/health
```

### View Token Claims (Decode JWT)
```bash
# Install jwt-cli: cargo install jwt-cli
echo $TOKEN | jwt decode -
```

### Test Rate Limiting
```bash
# Try 6 failed logins in quick succession (5 max in 15 min)
for i in {1..6}; do
  curl -X POST http://localhost:8080/api/auth/login \
    -H "Content-Type: application/json" \
    -d '{"username":"admin","password":"wrong"}' \
    -i
  echo ""
done
```

---

**Last Updated:** 2025-11-25
**API Version:** 1.0
**Base URL:** http://localhost:8080
