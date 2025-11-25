# Debugging Login in RustRover IDE

## Issues Found and Fixed

### 1. Bcrypt Hash Format Issue ✅ FIXED
**Problem:** The pre-generated bcrypt hash in the migration didn't work with the Rust bcrypt library.

**Root Cause:**
- Migration had hash: `$2a$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5NU3pY5pH3eBi`
- Rust bcrypt library couldn't verify this hash correctly

**Solution:**
- Generated new hash using Python bcrypt: `$2b$12$BZFHZuZf2AHVzag.lr00fO7YTtKzDjo9Hd3S8SBVX2rXXCBrV.W9u`
- Updated migration file: `migrations/20250101000009_seed_default_roles_and_admin.sql`

### 2. JSONB Type Mismatch ✅ FIXED
**Problem:** SQLx couldn't decode the `permissions` column from JSONB to `Vec<String>`

**Error:**
```
mismatched types; Rust type `alloc::vec::Vec<alloc::string::String>` (as SQL type `TEXT[]`)
is not compatible with SQL type `JSONB`
```

**Root Cause:**
- Database schema stores permissions as JSONB: `permissions JSONB NOT NULL`
- Rust model declared as `pub permissions: Vec<String>` without type hint
- SQLx assumed TEXT[] instead of JSONB

**Solution:**
- Added `#[sqlx(json)]` attribute to `permissions` field in `common/src/models.rs:574`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    #[sqlx(json)]  // ← Added this
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}
```

## Current Status

### ✅ Working Credentials
- **Username:** `admin`
- **Password:** `admin123`
- **Email:** `admin@example.com`
- **Role:** Admin (full permissions)

### Infrastructure Services Running
All dependencies are available for local debugging:
- PostgreSQL: `localhost:5432` (DB: `vietnam_cron`, User: `cronuser`, Pass: `cronpass`)
- Redis: `localhost:6379`
- NATS: `localhost:4222`
- MinIO: `localhost:9000` (Access: `minioadmin`, Secret: `minioadmin`)

### Application Services Stopped
Ready for debugging in RustRover:
- ❌ API (port 8080, 9090)
- ❌ Scheduler
- ❌ Worker

## Running API in RustRover for Debugging

### 1. Create Run Configuration

**Binary:** `api`
**Working Directory:** `/Users/linhdk1/Documents/rust-enterprise-cron`

### 2. Environment Variables

```bash
RUST_LOG=debug
DATABASE_URL=postgresql://cronuser:cronpass@localhost:5432/vietnam_cron
REDIS_URL=redis://localhost:6379
NATS_URL=nats://localhost:4222
MINIO_ENDPOINT=http://localhost:9000
MINIO_ACCESS_KEY=minioadmin
MINIO_SECRET_KEY=minioadmin
MINIO_BUCKET=vietnam-cron-jobs
MINIO_REGION=us-east-1
JWT_SECRET=your-secret-key-here-change-in-production
JWT_EXPIRATION_HOURS=24
AUTH_MODE=database
```

### 3. Debugging Breakpoints

Set breakpoints at:

1. **Login Handler** (`api/src/handlers/auth.rs:86`)
   ```rust
   pub async fn login(
       State(state): State<AppState>,
       headers: HeaderMap,
       Json(req): Json<LoginRequest>,
   ) -> Result<Json<SuccessResponse<LoginResponse>>, ErrorResponse>
   ```

2. **Password Verification** (`common/src/auth.rs:131`)
   ```rust
   let password_valid = bcrypt::verify(password, &user.password_hash)
   ```

3. **Permission Loading** (`common/src/auth.rs:142`)
   ```rust
   let permissions = self
       .user_repository
       .get_user_permissions(user.id)
       .await
   ```

4. **RBAC Middleware** (`api/src/middleware/rbac.rs:13`)
   ```rust
   pub async fn rbac_middleware(...)
   ```

### 4. Test the Login

**Via Browser:**
1. Navigate to http://localhost:8080/
2. Enter: `admin` / `admin123`
3. Click "Sign In"

**Via API:**
```bash
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}'
```

**Expected Response:**
```json
{
  "data": {
    "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
    "expires_at": 1732540800
  }
}
```

## Verification Steps

### Check Admin User in Database
```sql
SELECT username, email, enabled
FROM users
WHERE username = 'admin';
```

### Check Admin Roles
```sql
SELECT r.name, r.permissions
FROM roles r
INNER JOIN user_roles ur ON r.id = ur.role_id
WHERE ur.user_id = '00000000-0000-0000-0000-000000000001'::uuid;
```

Expected:
- Role: `Admin`
- Permissions: `["job:read", "job:write", "job:execute", "job:delete", ...]` (18 total)

### Test Permission Loading
```rust
// In your debug session, inspect:
// 1. user.password_hash value
// 2. bcrypt::verify() result
// 3. permissions Vec<String> content
```

## Common Debugging Scenarios

### Password Not Matching
**Check:**
1. Verify hash in database matches migration
2. Check bcrypt version compatibility
3. Ensure password string is trimmed

### Permissions Not Loading
**Check:**
1. `#[sqlx(json)]` attribute is present
2. JSONB format in database is valid
3. User has roles assigned in `user_roles` table

### Rate Limiting Blocking Login
**Clear Redis:**
```bash
docker-compose exec redis redis-cli FLUSHALL
```

## Useful SQL Queries

```sql
-- View all users with their roles
SELECT u.username, r.name as role, r.permissions
FROM users u
LEFT JOIN user_roles ur ON u.id = ur.user_id
LEFT JOIN roles r ON ur.role_id = r.id;

-- Reset admin password (if needed)
UPDATE users
SET password_hash = '$2b$12$BZFHZuZf2AHVzag.lr00fO7YTtKzDjo9Hd3S8SBVX2rXXCBrV.W9u'
WHERE username = 'admin';
```

## Tools

### Generate Bcrypt Hash
Use the included utility:
```bash
python3 << 'EOF'
import bcrypt
password = b"your_password"
salt = bcrypt.gensalt()
hashed = bcrypt.hashpw(password, salt)
print(f"Hash: {hashed.decode()}")
print(f"Verify: {bcrypt.checkpw(password, hashed)}")
EOF
```

Or use the Rust tool (if configured):
```rust
// tools/gen_hash.rs
cargo run --bin gen_hash your_password
```

## Next Steps

After fixing these issues, you should be able to:
1. ✅ Login with admin/admin123
2. ✅ Access the dashboard
3. ✅ See RBAC permissions working
4. ✅ Create/manage jobs
5. ✅ View executions

## Support

If you encounter other issues:
1. Check API logs for detailed error messages
2. Verify database schema matches migrations
3. Ensure all dependencies (Redis, NATS, etc.) are accessible
4. Review RUST_LOG output for authentication flow

---

**Last Updated:** 2025-11-24
**Status:** Both issues fixed, ready for debugging
