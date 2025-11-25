# Quick Start - API Testing

## 📦 Available Test Tools

I've created comprehensive API testing resources for you:

### 1. **Postman Collection** ⭐ RECOMMENDED
**File:** `Vietnam_Cron_API.postman_collection.json`

**Import to Postman:**
1. Open Postman
2. Click **Import** button
3. Select `Vietnam_Cron_API.postman_collection.json`
4. Collection will include all endpoints with automatic token management

**Features:**
- ✅ 20+ pre-configured requests
- ✅ Automatic JWT token extraction and storage
- ✅ Full RBAC endpoint coverage
- ✅ Detailed descriptions for each endpoint
- ✅ Example request/response bodies

### 2. **Bruno Collection**
**Folder:** `bruno-collection/`

**Import to Bruno:**
1. Open Bruno
2. Click **Open Collection**
3. Select the `bruno-collection` folder
4. Requests will be loaded

**Features:**
- ✅ Lightweight alternative to Postman
- ✅ Git-friendly (plain text files)
- ✅ Auto-saves JWT token to variables

### 3. **Curl Commands Reference**
**File:** `API_TEST_COMMANDS.md`

**Features:**
- ✅ Copy-paste ready curl commands
- ✅ Complete test flow script
- ✅ RBAC permissions reference
- ✅ Debugging tips

### 4. **Bash Test Script**
**File:** `/tmp/login_test.sh`

Run with:
```bash
/tmp/login_test.sh
```

---

## 🚀 Quick Test (1 Minute)

### Option A: Using Postman

1. **Import Collection:**
   ```
   File → Import → Select Vietnam_Cron_API.postman_collection.json
   ```

2. **Login:**
   - Open **Authentication → Login (Admin)**
   - Click **Send**
   - Token will be automatically saved

3. **Test Endpoints:**
   - Try **Jobs → List Jobs**
   - Try **Dashboard → Dashboard Home**
   - All subsequent requests will use the saved token

### Option B: Using Curl

```bash
# 1. Login and save token
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')

# 2. List jobs
curl -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN" | jq '.'

# 3. Access dashboard (with cookie)
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' \
  -c cookies.txt

curl -X GET http://localhost:8080/dashboard \
  -b cookies.txt
```

---

## 🔑 Default Credentials

| Username | Password   | Role          | Permissions |
|----------|------------|---------------|-------------|
| admin    | admin123   | Admin         | 18 (full)   |

---

## 📊 Key Endpoints to Test

### Authentication
- `POST /api/auth/login` - Login
- `POST /api/auth/refresh` - Refresh token

### Jobs
- `GET /api/jobs` - List jobs
- `POST /api/jobs` - Create job
- `POST /api/jobs/:id/trigger` - Trigger job

### Dashboard
- `GET /dashboard` - Main dashboard (HTML)

### Users (Admin Only)
- `GET /api/users` - List users
- `POST /api/users` - Create user
- `PUT /api/users/:id/roles` - Assign roles

---

## 🎯 Testing RBAC

### Test Admin Permissions
```bash
# Login as admin
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')

# Should succeed (admin has user:manage)
curl -X GET http://localhost:8080/api/users \
  -H "Authorization: Bearer $TOKEN"
```

### Test Permission Denial
```bash
# Create a regular user first (as admin)
curl -X POST http://localhost:8080/api/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "Test123!",
    "email": "test@example.com"
  }'

# Assign Regular User role
USER_ID="<get-from-response>"
curl -X PUT http://localhost:8080/api/users/$USER_ID/roles \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "role_ids": ["00000000-0000-0000-0000-000000000002"]
  }'

# Login as regular user
USER_TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"testuser","password":"Test123!"}' | \
  jq -r '.data.token')

# Should FAIL with 403 (regular user doesn't have user:manage)
curl -X GET http://localhost:8080/api/users \
  -H "Authorization: Bearer $USER_TOKEN"

# Should SUCCEED (regular user has job:read)
curl -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $USER_TOKEN"
```

---

## 🐛 Troubleshooting

### API Not Responding
```bash
# Check if API is running
curl http://localhost:8080/health

# If not running, start it:
docker-compose up api
# OR run locally:
cargo run --bin api
```

### 401 Unauthorized
```bash
# Token might be expired (24h), login again:
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')
```

### 403 Forbidden
- Check user permissions with:
```bash
# Decode JWT token to see permissions
echo $TOKEN | jwt decode -
# OR visit https://jwt.io and paste token
```

### Rate Limiting (429)
```bash
# Clear Redis rate limit:
docker-compose exec redis redis-cli FLUSHALL
```

---

## 📝 Notes

- **Cookie Auth:** Login sets an httpOnly cookie for browser access
- **Header Auth:** API clients should use `Authorization: Bearer <token>`
- **Token Expiry:** 24 hours (configurable via JWT_EXPIRATION_HOURS)
- **RBAC:** All protected endpoints check permissions automatically

---

## 🔗 Related Files

- `DEBUG_LOGIN.md` - Debugging guide for RustRover
- `API_TEST_COMMANDS.md` - Comprehensive curl reference
- `Vietnam_Cron_API.postman_collection.json` - Postman collection
- `bruno-collection/` - Bruno API client collection

---

**Last Updated:** 2025-11-25
**API Status:** ✅ Running locally for debugging
**Base URL:** http://localhost:8080
