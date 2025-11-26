# Rebuild Worker và Test Fix

## 📋 Tóm tắt thay đổi

**Vấn đề**: Worker nhận message từ NATS nhưng không xử lý job, chỉ log rồi return ngay.

**Giải pháp**: Refactor `WorkerJobConsumer` để tự quản lý handler thay vì nhận handler từ bên ngoài.

**Files đã sửa**:
- ✅ `common/src/worker/consumer.rs` - Constructor tự tạo handler với đầy đủ logic
- ✅ `worker/src/main.rs` - Đơn giản hóa, chỉ cần tạo WorkerJobConsumer

## 🔨 Bước 1: Rebuild Docker Image

```bash
# Rebuild worker image
docker-compose build worker

# Hoặc rebuild tất cả nếu có thay đổi common
docker-compose build
```

**Expected output**:
```
Building worker
[+] Building 120.5s (XX/XX) FINISHED
 => [internal] load build definition from Dockerfile
 => => transferring dockerfile: 3.45kB
 => [internal] load .dockerignore
 ...
 => => naming to docker.io/library/vietnam-cron:latest
```

## 🚀 Bước 2: Restart Worker

```bash
# Restart worker containers
docker-compose restart worker

# Verify workers are running
docker-compose ps | grep worker
```

**Expected output**:
```
rust-enterprise-cron-worker-1   vietnam-cron:latest   "worker"   Up X seconds   8080/tcp, 9090/tcp
rust-enterprise-cron-worker-2   vietnam-cron:latest   "worker"   Up X seconds   8080/tcp, 9090/tcp
```

## ✅ Bước 3: Verify Worker Logs

```bash
# Check worker logs
docker logs rust-enterprise-cron-worker-1 --tail 50
```

**Expected logs**:
```json
{"timestamp":"...","level":"INFO","message":"Starting Vietnam Enterprise Cron Worker"}
{"timestamp":"...","level":"INFO","message":"Configuration loaded successfully"}
{"timestamp":"...","level":"INFO","message":"Database pool initialized"}
{"timestamp":"...","level":"INFO","message":"MinIO client initialized"}
{"timestamp":"...","level":"INFO","message":"Executors initialized"}
{"timestamp":"...","level":"INFO","message":"NATS client initialized"}
{"timestamp":"...","level":"INFO","message":"Creating worker job consumer with MinIO integration"}
{"timestamp":"...","level":"INFO","message":"Creating NATS job consumer"}
{"timestamp":"...","level":"INFO","message":"Consumer created successfully"}
{"timestamp":"...","level":"INFO","message":"Worker consumer created, starting job processing"}
{"timestamp":"...","level":"INFO","message":"Starting worker job consumer"}
{"timestamp":"...","level":"INFO","message":"Consumer started, waiting for messages"}
{"timestamp":"...","level":"INFO","message":"Worker is running. Press Ctrl+C to shutdown gracefully"}
```

## 🧪 Bước 4: Test Job Execution

### Option A: Sử dụng test script (Recommended)

```bash
# Run automated test script
./test-worker-fix.sh
```

Script sẽ:
1. ✅ Check workers đang chạy
2. ✅ Lấy job ID từ database
3. ✅ Xóa executions cũ
4. ✅ Đợi scheduler trigger job
5. ✅ Verify execution được xử lý
6. ✅ Hiển thị logs và status

### Option B: Manual testing

#### 1. Xóa executions cũ (pending)
```bash
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron \
  -c "DELETE FROM job_executions WHERE status = 'pending';"
```

#### 2. Trigger job qua API
```bash
# Login và lấy token
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')

# Lấy job ID
JOB_ID=$(curl -s -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN" | \
  jq -r '.data[0].id')

# Trigger job
curl -X POST http://localhost:8080/api/jobs/$JOB_ID/trigger \
  -H "Authorization: Bearer $TOKEN"
```

#### 3. Monitor worker logs
```bash
# Follow worker logs
docker logs rust-enterprise-cron-worker-1 -f
```

**Expected logs khi xử lý job**:
```json
{"timestamp":"...","level":"INFO","message":"Processing message","stream_sequence":X}
{"timestamp":"...","level":"INFO","message":"Deserialized job message","execution_id":"...","job_id":"..."}
{"timestamp":"...","level":"INFO","message":"Processing job"}
{"timestamp":"...","level":"INFO","message":"No existing execution found, proceeding with job execution"}
{"timestamp":"...","level":"INFO","message":"Loaded job metadata from database","job_name":"..."}
{"timestamp":"...","level":"INFO","message":"Loaded and parsed job definition from MinIO","step_count":X}
{"timestamp":"...","level":"INFO","message":"Executing job steps sequentially"}
{"timestamp":"...","level":"INFO","message":"Executing step","step_index":0,"step_id":"..."}
{"timestamp":"...","level":"INFO","message":"Step completed successfully"}
{"timestamp":"...","level":"INFO","message":"Context saved to MinIO after step completion"}
{"timestamp":"...","level":"INFO","message":"All steps completed successfully"}
{"timestamp":"...","level":"INFO","message":"Job execution completed successfully"}
{"timestamp":"...","level":"INFO","message":"Final job context saved to MinIO successfully"}
{"timestamp":"...","level":"INFO","message":"Job processed successfully"}
{"timestamp":"...","level":"INFO","message":"Message acknowledged"}
```

#### 4. Check execution status
```bash
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron \
  -c "SELECT id, status, started_at, completed_at, error FROM job_executions ORDER BY created_at DESC LIMIT 3;"
```

**Expected result**:
```
                  id                  | status  |         started_at         |        completed_at        | error
--------------------------------------+---------+----------------------------+----------------------------+-------
 xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx | success | 2025-11-25 14:30:00.123+00 | 2025-11-25 14:30:02.456+00 | 
```

## 🎯 Success Criteria

✅ **Worker logs có**:
- "Processing job"
- "Loaded job metadata from database"
- "Loaded and parsed job definition from MinIO"
- "Executing step"
- "Job execution completed successfully"

✅ **Execution status**:
- Chuyển từ `pending` → `running` → `success` (hoặc `failed` nếu có lỗi)
- `started_at` và `completed_at` được set
- Không còn stuck ở `pending`

✅ **Dashboard**:
- Tab "Executions" hiển thị execution mới
- Status hiển thị đúng (success/failed)
- Last Run được update

## ❌ Troubleshooting

### Vấn đề 1: Worker không start
```bash
# Check worker logs
docker logs rust-enterprise-cron-worker-1

# Common issues:
# - MinIO connection failed → Check MinIO is running
# - Database connection failed → Check PostgreSQL is running
# - NATS connection failed → Check NATS is running
```

### Vấn đề 2: Execution vẫn pending
```bash
# Check if worker is consuming messages
docker logs rust-enterprise-cron-worker-1 | grep "Processing message"

# Check NATS stream
curl -s http://localhost:8222/jsz?streams=1 | python3 -m json.tool | grep -A 5 "messages"

# If no messages, check scheduler
docker logs vietnam-cron-scheduler | grep "published"
```

### Vấn đề 3: Job failed
```bash
# Check error message
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron \
  -c "SELECT id, error FROM job_executions WHERE status = 'failed' ORDER BY created_at DESC LIMIT 1;"

# Common errors:
# - "Failed to load job definition from MinIO" → Job definition not uploaded
# - "Step execution failed" → Check step configuration
# - "Timeout" → Increase job timeout_seconds
```

## 📊 Monitoring

### Real-time monitoring
```bash
# Terminal 1: Worker logs
docker logs rust-enterprise-cron-worker-1 -f

# Terminal 2: Scheduler logs
docker logs vietnam-cron-scheduler -f

# Terminal 3: Database queries
watch -n 2 'docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -c "SELECT status, COUNT(*) FROM job_executions GROUP BY status;"'
```

### Dashboard monitoring
1. Open http://localhost:8080/dashboard
2. Click "Executions" tab
3. Refresh để xem executions mới
4. Click vào execution để xem chi tiết

## 🎉 Expected Final State

Sau khi fix thành công:

1. ✅ Worker consume messages từ NATS
2. ✅ Worker xử lý jobs với đầy đủ logic:
   - Load job từ database
   - Load job definition từ MinIO
   - Execute steps sequentially
   - Update execution status
   - Save context to MinIO
3. ✅ Executions không còn stuck ở pending
4. ✅ Dashboard hiển thị execution history đầy đủ
5. ✅ Jobs chạy theo schedule hoặc manual trigger

---

**Tạo bởi**: Worker Fix Refactoring
**Ngày**: 2025-11-25
**Version**: 1.0
