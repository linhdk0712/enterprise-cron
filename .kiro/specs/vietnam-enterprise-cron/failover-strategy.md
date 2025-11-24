# Failover Strategy - PostgreSQL & MinIO Resilience

## Tổng Quan

Tài liệu này mô tả chiến lược dự phòng (failover/fallback) khi PostgreSQL hoặc MinIO gặp sự cố, đảm bảo hệ thống Vietnam Enterprise Cron vẫn hoạt động với degraded mode thay vì hoàn toàn down.

## Mục Tiêu

1. **Zero Downtime**: Hệ thống không bao giờ hoàn toàn ngừng hoạt động
2. **Graceful Degradation**: Giảm chức năng thay vì crash hoàn toàn
3. **Data Consistency**: Không mất dữ liệu khi failover
4. **Auto Recovery**: Tự động phục hồi khi service khả dụng trở lại
5. **Observability**: Monitoring và alerting rõ ràng về trạng thái failover

## Kiến Trúc Tổng Thể

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │Scheduler │  │  Worker  │  │   API    │                  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                  │
└───────┼─────────────┼─────────────┼────────────────────────┘
        │             │             │
        ▼             ▼             ▼
┌─────────────────────────────────────────────────────────────┐
│              Resilience Layer (Circuit Breakers)            │
│  ┌──────────────────┐      ┌──────────────────┐            │
│  │ PostgreSQL CB    │      │   MinIO CB       │            │
│  │ + Health Check   │      │ + Health Check   │            │
│  └────┬─────────────┘      └────┬─────────────┘            │
└───────┼──────────────────────────┼──────────────────────────┘
        │                          │
        ▼                          ▼
┌──────────────────┐      ┌──────────────────┐
│   PostgreSQL     │      │      MinIO       │
│   Primary        │      │     Primary      │
│      ↓           │      │        ↓         │
│   Read Replica   │      │   Replication    │
└──────────────────┘      └──────────────────┘
        ↓                          ↓
┌──────────────────┐      ┌──────────────────┐
│  Fallback Cache  │      │  Fallback Cache  │
│  (Redis/Local)   │      │  (Local Disk)    │
└──────────────────┘      └──────────────────┘
```


---

## 1. PostgreSQL Failover Strategy

### 1.1 Kiến Trúc PostgreSQL High Availability

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Tier                          │
│         (Scheduler, Worker, API với Connection Pool)         │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              PgBouncer (Connection Pooler)                   │
│              + Health Check + Auto Failover                  │
└────────────────────────┬────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ PostgreSQL   │  │ PostgreSQL   │  │ PostgreSQL   │
│   Primary    │──│ Read Replica │  │ Read Replica │
│  (Write)     │  │   (Read)     │  │   (Read)     │
└──────┬───────┘  └──────────────┘  └──────────────┘
       │
       │ WAL Streaming Replication
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│              Redis Cache (Fallback Layer)                     │
│  - Cached job definitions (TTL: 5 minutes)                   │
│  - Cached execution status (TTL: 1 minute)                   │
│  - Cached variables (TTL: 10 minutes)                        │
└──────────────────────────────────────────────────────────────┘
```

### 1.2 Failure Scenarios & Solutions

#### Scenario 1: Primary PostgreSQL Down

**Detection:**
```rust
// Health check mỗi 5 giây
async fn check_postgres_health(pool: &PgPool) -> HealthStatus {
    match sqlx::query("SELECT 1").fetch_one(pool).await {
        Ok(_) => HealthStatus::Healthy,
        Err(e) => {
            tracing::error!(error = %e, "PostgreSQL health check failed");
            HealthStatus::Unhealthy
        }
    }
}
```

**Failover Actions:**
1. **Automatic Promotion**: Patroni/Stolon tự động promote read replica lên primary
2. **Connection Rerouting**: PgBouncer chuyển connections sang primary mới
3. **Circuit Breaker**: Mở circuit breaker trong 30 giây để tránh connection storm

**Degraded Mode Operations:**
- ✅ **Scheduler**: Đọc job definitions từ Redis cache
- ✅ **Worker**: Tiếp tục execute jobs đã queue trong NATS
- ⚠️ **API**: Read-only mode, không cho phép create/update jobs
- ❌ **New Job Scheduling**: Tạm dừng schedule jobs mới


#### Scenario 2: All PostgreSQL Instances Down (Disaster)

**Fallback Strategy: Redis Cache + Local Disk**

```rust
pub struct ResilientJobRepository {
    primary: PgPool,
    cache: RedisPool,
    local_backup: PathBuf,
    circuit_breaker: CircuitBreaker,
}

impl ResilientJobRepository {
    async fn find_jobs_due(&self, now: DateTime<Utc>) -> Result<Vec<Job>> {
        // Try 1: PostgreSQL (with circuit breaker)
        if self.circuit_breaker.is_closed() {
            match self.find_from_postgres(now).await {
                Ok(jobs) => {
                    self.cache_jobs(&jobs).await?; // Cache for fallback
                    return Ok(jobs);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "PostgreSQL unavailable");
                    self.circuit_breaker.record_failure();
                }
            }
        }
        
        // Try 2: Redis Cache
        match self.find_from_cache(now).await {
            Ok(jobs) => {
                tracing::warn!("Using cached jobs from Redis (degraded mode)");
                return Ok(jobs);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Redis cache unavailable");
            }
        }
        
        // Try 3: Local Disk Backup (last resort)
        match self.find_from_local_backup(now).await {
            Ok(jobs) => {
                tracing::error!("Using local disk backup (critical degraded mode)");
                Ok(jobs)
            }
            Err(e) => {
                tracing::error!(error = %e, "All storage backends unavailable");
                Err(e)
            }
        }
    }
}
```

**Degraded Mode Capabilities:**
- ✅ **Execute Existing Jobs**: Jobs đã được cache vẫn chạy bình thường
- ✅ **Read Job Status**: Đọc từ cache (có thể stale)
- ❌ **Create New Jobs**: Không thể tạo jobs mới
- ❌ **Update Jobs**: Không thể update jobs
- ❌ **Execution History**: Không ghi được lịch sử mới

**Data Consistency:**
```rust
// Queue execution updates để replay sau khi PostgreSQL khả dụng
struct ExecutionUpdate {
    execution_id: Uuid,
    status: ExecutionStatus,
    timestamp: DateTime<Utc>,
    result: Option<String>,
}

// Store trong NATS JetStream với retention
async fn queue_execution_update(update: ExecutionUpdate) -> Result<()> {
    let subject = format!("execution.updates.{}", update.execution_id);
    nats_client.publish(subject, serde_json::to_vec(&update)?).await?;
    Ok(())
}

// Replay khi PostgreSQL khả dụng trở lại
async fn replay_pending_updates(pool: &PgPool, nats: &NatsClient) -> Result<()> {
    let consumer = nats.subscribe("execution.updates.*").await?;
    
    while let Some(msg) = consumer.next().await {
        let update: ExecutionUpdate = serde_json::from_slice(&msg.data)?;
        
        sqlx::query!(
            "INSERT INTO job_executions (...) VALUES (...) 
             ON CONFLICT (id) DO UPDATE SET status = $1, result = $2",
            update.status.to_string(),
            update.result
        ).execute(pool).await?;
        
        msg.ack().await?;
    }
    
    Ok(())
}
```


#### Scenario 3: Read Replicas Down (Primary Still Up)

**Solution: Automatic Fallback to Primary**

```rust
pub struct SmartPgPool {
    primary: PgPool,
    replicas: Vec<PgPool>,
    current_replica_idx: AtomicUsize,
}

impl SmartPgPool {
    async fn execute_read_query<T>(&self, query: Query) -> Result<T> {
        // Try replicas first (round-robin)
        for _ in 0..self.replicas.len() {
            let idx = self.current_replica_idx.fetch_add(1, Ordering::Relaxed);
            let replica = &self.replicas[idx % self.replicas.len()];
            
            match query.fetch_one(replica).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(replica_idx = idx, error = %e, "Replica unavailable");
                    continue;
                }
            }
        }
        
        // Fallback to primary
        tracing::warn!("All replicas down, using primary for read");
        query.fetch_one(&self.primary).await
    }
}
```

### 1.3 Cache Strategy

**Redis Cache Structure:**
```rust
// Cache keys với TTL
const CACHE_KEY_JOB: &str = "job:{job_id}";           // TTL: 5 minutes
const CACHE_KEY_JOBS_DUE: &str = "jobs:due:{minute}"; // TTL: 1 minute
const CACHE_KEY_VARIABLES: &str = "vars:{scope}";     // TTL: 10 minutes
const CACHE_KEY_EXECUTION: &str = "exec:{exec_id}";   // TTL: 30 seconds

// Write-through cache
async fn save_job(&self, job: &Job) -> Result<()> {
    // 1. Write to PostgreSQL
    sqlx::query!("INSERT INTO jobs (...) VALUES (...)")
        .execute(&self.pool).await?;
    
    // 2. Update cache
    let cache_key = format!("job:{}", job.id);
    let job_json = serde_json::to_string(job)?;
    self.redis.set_ex(cache_key, job_json, 300).await?;
    
    Ok(())
}

// Read-through cache
async fn find_job(&self, id: Uuid) -> Result<Option<Job>> {
    let cache_key = format!("job:{}", id);
    
    // Try cache first
    if let Ok(Some(cached)) = self.redis.get::<String>(cache_key.clone()).await {
        return Ok(Some(serde_json::from_str(&cached)?));
    }
    
    // Cache miss, read from PostgreSQL
    let job = sqlx::query_as!(Job, "SELECT * FROM jobs WHERE id = $1", id)
        .fetch_optional(&self.pool).await?;
    
    // Update cache
    if let Some(ref j) = job {
        let job_json = serde_json::to_string(j)?;
        self.redis.set_ex(cache_key, job_json, 300).await.ok();
    }
    
    Ok(job)
}
```

**Local Disk Backup:**
```rust
// Định kỳ backup critical data ra disk mỗi 1 phút
async fn backup_to_disk(&self) -> Result<()> {
    let jobs = sqlx::query_as!(Job, "SELECT * FROM jobs WHERE enabled = true")
        .fetch_all(&self.pool).await?;
    
    let backup_path = self.local_backup.join("jobs_backup.json");
    let backup_data = serde_json::to_string_pretty(&jobs)?;
    
    tokio::fs::write(backup_path, backup_data).await?;
    
    tracing::info!(job_count = jobs.len(), "Backed up jobs to local disk");
    Ok(())
}
```


---

## 2. MinIO Failover Strategy

### 2.1 Kiến Trúc MinIO High Availability

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Tier                          │
│         (Worker với MinIO Client + Circuit Breaker)          │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              MinIO Client với Retry Logic                    │
│              + Health Check + Circuit Breaker                │
└────────────────────────┬────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   MinIO      │  │   MinIO      │  │   MinIO      │
│   Node 1     │──│   Node 2     │──│   Node 3     │
│ (Distributed)│  │ (Distributed)│  │ (Distributed)│
└──────────────┘  └──────────────┘  └──────────────┘
       │                │                │
       └────────────────┼────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              Fallback Storage Layers                         │
│  ┌──────────────────┐      ┌──────────────────┐            │
│  │  Redis Cache     │      │  Local Disk      │            │
│  │  (Hot Data)      │      │  (Cold Backup)   │            │
│  └──────────────────┘      └──────────────────┘            │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Failure Scenarios & Solutions

#### Scenario 1: Single MinIO Node Down

**Solution: MinIO Distributed Mode tự động xử lý**

MinIO distributed mode (4+ nodes) có erasure coding, tự động phục hồi khi 1-2 nodes down.

```toml
# docker-compose.yml
services:
  minio1:
    image: minio/minio
    command: server http://minio{1...4}/data --console-address ":9001"
    
  minio2:
    image: minio/minio
    command: server http://minio{1...4}/data --console-address ":9001"
    
  minio3:
    image: minio/minio
    command: server http://minio{1...4}/data --console-address ":9001"
    
  minio4:
    image: minio/minio
    command: server http://minio{1...4}/data --console-address ":9001"
```

**Application không cần thay đổi gì**, MinIO SDK tự động retry.


#### Scenario 2: All MinIO Nodes Down (Disaster)

**Fallback Strategy: Multi-Tier Storage**

```rust
pub struct ResilientMinIOClient {
    primary: MinIOClient,
    redis_cache: RedisPool,
    local_disk: PathBuf,
    circuit_breaker: CircuitBreaker,
}

impl ResilientMinIOClient {
    /// Load job definition với fallback chain
    async fn load_job_definition(&self, job_id: Uuid) -> Result<JobDefinition> {
        let path = format!("jobs/{}/definition.json", job_id);
        
        // Try 1: MinIO (with circuit breaker)
        if self.circuit_breaker.is_closed() {
            match self.load_from_minio(&path).await {
                Ok(data) => {
                    // Cache for fallback
                    self.cache_to_redis(&path, &data).await.ok();
                    self.cache_to_disk(&path, &data).await.ok();
                    return Ok(serde_json::from_slice(&data)?);
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %path, "MinIO unavailable");
                    self.circuit_breaker.record_failure();
                }
            }
        }
        
        // Try 2: Redis Cache (hot cache, TTL: 10 minutes)
        match self.load_from_redis(&path).await {
            Ok(data) => {
                tracing::warn!(path = %path, "Using cached job definition from Redis");
                return Ok(serde_json::from_slice(&data)?);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Redis cache miss");
            }
        }
        
        // Try 3: Local Disk (cold backup)
        match self.load_from_disk(&path).await {
            Ok(data) => {
                tracing::error!(path = %path, "Using local disk backup (critical mode)");
                Ok(serde_json::from_slice(&data)?)
            }
            Err(e) => {
                tracing::error!(error = %e, path = %path, "All storage backends failed");
                Err(anyhow!("Job definition unavailable: {}", path))
            }
        }
    }
    
    /// Store job definition với write-through cache
    async fn store_job_definition(
        &self, 
        job_id: Uuid, 
        definition: &JobDefinition
    ) -> Result<String> {
        let path = format!("jobs/{}/definition.json", job_id);
        let data = serde_json::to_vec_pretty(definition)?;
        
        // Try MinIO first
        match self.store_to_minio(&path, &data).await {
            Ok(etag) => {
                // Success, update caches
                self.cache_to_redis(&path, &data).await.ok();
                self.cache_to_disk(&path, &data).await.ok();
                return Ok(etag);
            }
            Err(e) => {
                tracing::error!(error = %e, "MinIO write failed");
                self.circuit_breaker.record_failure();
            }
        }
        
        // MinIO down, queue for later sync
        self.queue_pending_write(&path, &data).await?;
        
        // Store to fallback layers
        self.cache_to_redis(&path, &data).await?;
        self.cache_to_disk(&path, &data).await?;
        
        tracing::warn!(path = %path, "Job definition stored to fallback only");
        Ok("fallback".to_string())
    }
}
```

**Pending Writes Queue:**
```rust
// Queue writes trong NATS để replay sau
struct PendingWrite {
    path: String,
    data: Vec<u8>,
    timestamp: DateTime<Utc>,
    retry_count: u32,
}

async fn queue_pending_write(&self, path: &str, data: &[u8]) -> Result<()> {
    let pending = PendingWrite {
        path: path.to_string(),
        data: data.to_vec(),
        timestamp: Utc::now(),
        retry_count: 0,
    };
    
    let subject = format!("minio.pending.writes.{}", Uuid::new_v4());
    self.nats.publish(subject, serde_json::to_vec(&pending)?).await?;
    
    tracing::warn!(path = %path, "Queued write for later sync");
    Ok(())
}

// Background task replay pending writes
async fn replay_pending_writes(&self) -> Result<()> {
    let consumer = self.nats.subscribe("minio.pending.writes.*").await?;
    
    while let Some(msg) = consumer.next().await {
        let mut pending: PendingWrite = serde_json::from_slice(&msg.data)?;
        
        match self.store_to_minio(&pending.path, &pending.data).await {
            Ok(_) => {
                tracing::info!(path = %pending.path, "Replayed pending write");
                msg.ack().await?;
            }
            Err(e) => {
                pending.retry_count += 1;
                if pending.retry_count >= 10 {
                    tracing::error!(
                        path = %pending.path, 
                        "Failed to replay write after 10 attempts"
                    );
                    msg.ack().await?; // Give up
                } else {
                    tracing::warn!(
                        path = %pending.path, 
                        retry = pending.retry_count,
                        "Retry pending write later"
                    );
                    msg.nak().await?; // Retry later
                }
            }
        }
    }
    
    Ok(())
}
```


#### Scenario 3: Job Context Storage Failure

**Critical**: Job Context chứa step outputs, cần thiết cho multi-step jobs.

**Solution: Hybrid Storage**

```rust
pub struct JobContextManager {
    minio: ResilientMinIOClient,
    redis: RedisPool,
    local_disk: PathBuf,
}

impl JobContextManager {
    /// Load context với aggressive caching
    async fn load_context(&self, execution_id: Uuid) -> Result<JobContext> {
        let path = format!("jobs/*/executions/{}/context.json", execution_id);
        
        // Try memory cache first (trong Worker process)
        if let Some(ctx) = self.memory_cache.get(&execution_id) {
            return Ok(ctx.clone());
        }
        
        // Try Redis (hot cache)
        if let Ok(data) = self.redis.get::<Vec<u8>>(&path).await {
            let ctx: JobContext = serde_json::from_slice(&data)?;
            self.memory_cache.insert(execution_id, ctx.clone());
            return Ok(ctx);
        }
        
        // Try MinIO
        if let Ok(data) = self.minio.load_from_minio(&path).await {
            let ctx: JobContext = serde_json::from_slice(&data)?;
            self.cache_context(&execution_id, &ctx).await?;
            return Ok(ctx);
        }
        
        // Try local disk
        if let Ok(data) = self.load_from_disk(&path).await {
            let ctx: JobContext = serde_json::from_slice(&data)?;
            self.cache_context(&execution_id, &ctx).await?;
            return Ok(ctx);
        }
        
        Err(anyhow!("Job context unavailable: {}", execution_id))
    }
    
    /// Save context với triple-write
    async fn save_context(&self, context: &JobContext) -> Result<()> {
        let path = format!(
            "jobs/{}/executions/{}/context.json",
            context.job_id,
            context.execution_id
        );
        let data = serde_json::to_vec_pretty(context)?;
        
        // Write to all layers simultaneously (fire and forget for fallbacks)
        let minio_write = self.minio.store_to_minio(&path, &data);
        let redis_write = self.cache_to_redis(&path, &data, 600); // 10 min TTL
        let disk_write = self.cache_to_disk(&path, &data);
        
        // Wait for at least one success
        tokio::select! {
            Ok(_) = minio_write => {
                tracing::debug!("Context saved to MinIO");
            }
            Ok(_) = redis_write => {
                tracing::warn!("Context saved to Redis only (MinIO slow/down)");
            }
            Ok(_) = disk_write => {
                tracing::error!("Context saved to disk only (MinIO+Redis down)");
            }
            else => {
                return Err(anyhow!("Failed to save context to any storage"));
            }
        }
        
        // Update memory cache
        self.memory_cache.insert(context.execution_id, context.clone());
        
        Ok(())
    }
}
```

**Degraded Mode Behavior:**
- ✅ **Continue Execution**: Jobs tiếp tục chạy với cached context
- ✅ **Step Output Storage**: Lưu vào Redis/Disk thay vì MinIO
- ⚠️ **Context Size Limit**: Redis có giới hạn 512MB, cần compress nếu context lớn
- ❌ **Large File Processing**: Không thể xử lý files >100MB nếu MinIO down


### 2.3 File Processing Fallback

**Problem**: File processing jobs cần đọc/ghi Excel/CSV từ MinIO.

**Solution: Temporary Local Storage**

```rust
pub struct FileProcessingExecutor {  
  minio: ResilientMinIOClient,
    local_temp: PathBuf,
}

impl FileProcessingExecutor {
    async fn execute_file_read(&self, step: &FileProcessingStep) -> Result<StepOutput> {
        // Try MinIO first
        match self.minio.load_file(&step.source_path).await {
            Ok(data) => {
                return self.process_file_data(&data, step).await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "MinIO unavailable for file read");
            }
        }
        
        // Fallback: Check local temp storage
        let local_path = self.local_temp.join(&step.source_path);
        if local_path.exists() {
            tracing::warn!("Using local temp file (MinIO down)");
            let data = tokio::fs::read(&local_path).await?;
            return self.process_file_data(&data, step).await;
        }
        
        Err(anyhow!("File unavailable: {}", step.source_path))
    }
    
    async fn execute_file_write(&self, data: &[u8], path: &str) -> Result<()> {
        // Write to local temp first (fast)
        let local_path = self.local_temp.join(path);
        tokio::fs::create_dir_all(local_path.parent().unwrap()).await?;
        tokio::fs::write(&local_path, data).await?;
        
        // Try MinIO (async, non-blocking)
        match self.minio.store_file(path, data).await {
            Ok(_) => {
                tracing::debug!("File written to MinIO");
                // Clean up local temp after successful upload
                tokio::fs::remove_file(&local_path).await.ok();
            }
            Err(e) => {
                tracing::warn!(error = %e, "MinIO write failed, keeping local copy");
                // Queue for later sync
                self.queue_file_upload(path, &local_path).await?;
            }
        }
        
        Ok(())
    }
}
```

**Degraded Mode:**
- ✅ **Read Cached Files**: Đọc files từ local temp nếu MinIO down
- ✅ **Write to Local**: Ghi files vào local disk, sync sau
- ⚠️ **Disk Space**: Cần monitor disk space cho temp files
- ❌ **Cross-Worker Access**: Workers khác không thấy files trong local temp


---

## 3. Circuit Breaker Implementation

### 3.1 Circuit Breaker States

```rust
pub enum CircuitState {
    Closed,      // Normal operation
    Open,        // Failing, reject requests immediately
    HalfOpen,    // Testing if service recovered
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<AtomicU32>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    config: CircuitBreakerConfig,
}

pub struct CircuitBreakerConfig {
    failure_threshold: u32,      // Open after N failures (default: 5)
    timeout: Duration,           // Stay open for X seconds (default: 30s)
    half_open_max_calls: u32,    // Test with N calls (default: 3)
}
```

### 3.2 Circuit Breaker Logic

```rust
impl CircuitBreaker {
    pub async fn call<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        // Check current state
        let state = self.state.read().await.clone();
        
        match state {
            CircuitState::Open => {
                // Check if timeout expired
                if self.should_attempt_reset().await {
                    self.transition_to_half_open().await;
                } else {
                    return Err(anyhow!("Circuit breaker is OPEN"));
                }
            }
            CircuitState::HalfOpen => {
                // Limit concurrent test calls
                if !self.can_attempt_half_open().await {
                    return Err(anyhow!("Circuit breaker is HALF_OPEN (testing)"));
                }
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }
        
        // Execute the function
        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }
    
    async fn on_success(&self) {
        let state = self.state.read().await.clone();
        
        match state {
            CircuitState::HalfOpen => {
                // Success in half-open, close the circuit
                tracing::info!("Circuit breaker: HALF_OPEN -> CLOSED (recovered)");
                *self.state.write().await = CircuitState::Closed;
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            _ => {}
        }
    }
    
    async fn on_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.write().await = Some(Instant::now());
        
        if failures >= self.config.failure_threshold {
            let mut state = self.state.write().await;
            if matches!(*state, CircuitState::Closed | CircuitState::HalfOpen) {
                tracing::warn!(
                    failures = failures,
                    "Circuit breaker: OPEN (threshold exceeded)"
                );
                *state = CircuitState::Open;
            }
        }
    }
}
```


---

## 4. Health Check & Monitoring

### 4.1 Health Check Endpoints

```rust
#[derive(Serialize)]
pub struct HealthStatus {
    status: String,              // "healthy", "degraded", "unhealthy"
    postgres: ComponentHealth,
    minio: ComponentHealth,
    redis: ComponentHealth,
    nats: ComponentHealth,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ComponentHealth {
    status: String,
    latency_ms: Option<u64>,
    circuit_breaker_state: String,
    last_error: Option<String>,
    fallback_active: bool,
}

async fn health_check_handler(
    State(app_state): State<AppState>
) -> Json<HealthStatus> {
    let postgres_health = check_postgres(&app_state.db_pool).await;
    let minio_health = check_minio(&app_state.minio_client).await;
    let redis_health = check_redis(&app_state.redis_pool).await;
    let nats_health = check_nats(&app_state.nats_client).await;
    
    let overall_status = if postgres_health.is_healthy() 
        && minio_health.is_healthy() {
        "healthy"
    } else if postgres_health.is_degraded() || minio_health.is_degraded() {
        "degraded"
    } else {
        "unhealthy"
    };
    
    Json(HealthStatus {
        status: overall_status.to_string(),
        postgres: postgres_health,
        minio: minio_health,
        redis: redis_health,
        nats: nats_health,
        timestamp: Utc::now(),
    })
}
```

### 4.2 Prometheus Metrics

```rust
// Define metrics
lazy_static! {
    static ref POSTGRES_HEALTH: IntGauge = register_int_gauge!(
        "postgres_health_status",
        "PostgreSQL health status (1=healthy, 0=unhealthy)"
    ).unwrap();
    
    static ref MINIO_HEALTH: IntGauge = register_int_gauge!(
        "minio_health_status",
        "MinIO health status (1=healthy, 0=unhealthy)"
    ).unwrap();
    
    static ref CIRCUIT_BREAKER_STATE: IntGaugeVec = register_int_gauge_vec!(
        "circuit_breaker_state",
        "Circuit breaker state (0=closed, 1=open, 2=half_open)",
        &["component"]
    ).unwrap();
    
    static ref FALLBACK_ACTIVE: IntGaugeVec = register_int_gauge_vec!(
        "fallback_storage_active",
        "Whether fallback storage is being used (1=yes, 0=no)",
        &["component", "fallback_type"]
    ).unwrap();
    
    static ref PENDING_SYNC_QUEUE_SIZE: IntGauge = register_int_gauge!(
        "pending_sync_queue_size",
        "Number of pending writes waiting to sync"
    ).unwrap();
}

// Update metrics
async fn update_health_metrics() {
    loop {
        // PostgreSQL
        match check_postgres_health(&pool).await {
            HealthStatus::Healthy => POSTGRES_HEALTH.set(1),
            _ => POSTGRES_HEALTH.set(0),
        }
        
        // MinIO
        match check_minio_health(&client).await {
            HealthStatus::Healthy => MINIO_HEALTH.set(1),
            _ => MINIO_HEALTH.set(0),
        }
        
        // Circuit breakers
        CIRCUIT_BREAKER_STATE
            .with_label_values(&["postgres"])
            .set(postgres_cb.state_as_int());
        
        CIRCUIT_BREAKER_STATE
            .with_label_values(&["minio"])
            .set(minio_cb.state_as_int());
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```


### 4.3 Alerting Rules

```yaml
# Prometheus alerting rules
groups:
  - name: vietnam_cron_failover
    interval: 30s
    rules:
      - alert: PostgreSQLDown
        expr: postgres_health_status == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "PostgreSQL is down"
          description: "System is running in degraded mode using cache"
      
      - alert: MinIODown
        expr: minio_health_status == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "MinIO is down"
          description: "File operations using fallback storage"
      
      - alert: CircuitBreakerOpen
        expr: circuit_breaker_state{component=~"postgres|minio"} == 1
        for: 30s
        labels:
          severity: warning
        annotations:
          summary: "Circuit breaker opened for {{ $labels.component }}"
          description: "Requests are being rejected to prevent cascading failures"
      
      - alert: FallbackStorageActive
        expr: fallback_storage_active == 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Fallback storage active for {{ $labels.component }}"
          description: "System degraded, using {{ $labels.fallback_type }}"
      
      - alert: PendingSyncQueueHigh
        expr: pending_sync_queue_size > 100
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High number of pending sync operations"
          description: "{{ $value }} writes waiting to sync to primary storage"
```


---

## 5. Auto Recovery Mechanisms

### 5.1 PostgreSQL Recovery

```rust
pub struct PostgreSQLRecoveryManager {
    pool: PgPool,
    cache: RedisPool,
    nats: NatsClient,
    recovery_state: Arc<RwLock<RecoveryState>>,
}

impl PostgreSQLRecoveryManager {
    /// Background task kiểm tra và phục hồi
    pub async fn run_recovery_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            if self.is_postgres_healthy().await {
                if self.recovery_state.read().await.needs_recovery() {
                    tracing::info!("PostgreSQL recovered, starting data sync");
                    
                    if let Err(e) = self.sync_pending_data().await {
                        tracing::error!(error = %e, "Recovery sync failed");
                    } else {
                        tracing::info!("Recovery sync completed successfully");
                        self.recovery_state.write().await.mark_recovered();
                    }
                }
            }
        }
    }
    
    async fn sync_pending_data(&self) -> Result<()> {
        // 1. Replay execution updates từ NATS
        self.replay_execution_updates().await?;
        
        // 2. Sync job definitions từ cache
        self.sync_job_definitions().await?;
        
        // 3. Verify data consistency
        self.verify_consistency().await?;
        
        Ok(())
    }
    
    async fn replay_execution_updates(&self) -> Result<()> {
        let consumer = self.nats
            .subscribe("execution.updates.*")
            .await?;
        
        let mut count = 0;
        while let Some(msg) = consumer.next().await {
            let update: ExecutionUpdate = serde_json::from_slice(&msg.data)?;
            
            sqlx::query!(
                "INSERT INTO job_executions (...) VALUES (...)
                 ON CONFLICT (id) DO UPDATE SET 
                 status = EXCLUDED.status,
                 result = EXCLUDED.result,
                 completed_at = EXCLUDED.completed_at"
            )
            .execute(&self.pool)
            .await?;
            
            msg.ack().await?;
            count += 1;
        }
        
        tracing::info!(count = count, "Replayed execution updates");
        Ok(())
    }
}
```

### 5.2 MinIO Recovery

```rust
pub struct MinIORecoveryManager {
    minio: MinIOClient,
    local_disk: PathBuf,
    nats: NatsClient,
}

impl MinIORecoveryManager {
    pub async fn run_recovery_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            
            if self.is_minio_healthy().await {
                if let Err(e) = self.sync_pending_writes().await {
                    tracing::error!(error = %e, "MinIO sync failed");
                } else {
                    tracing::debug!("MinIO sync completed");
                }
            }
        }
    }
    
    async fn sync_pending_writes(&self) -> Result<()> {
        // 1. Sync từ NATS queue
        let consumer = self.nats
            .subscribe("minio.pending.writes.*")
            .await?;
        
        let mut synced = 0;
        while let Some(msg) = consumer.next().await {
            let pending: PendingWrite = serde_json::from_slice(&msg.data)?;
            
            match self.minio.put_object(&pending.path, &pending.data).await {
                Ok(_) => {
                    msg.ack().await?;
                    synced += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %pending.path,
                        "Failed to sync, will retry"
                    );
                    msg.nak().await?;
                    break; // Stop and retry later
                }
            }
        }
        
        // 2. Sync local temp files
        let local_files = self.scan_local_temp_files().await?;
        for file_path in local_files {
            match self.upload_local_file(&file_path).await {
                Ok(_) => {
                    tokio::fs::remove_file(&file_path).await?;
                    synced += 1;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to upload local file");
                }
            }
        }
        
        if synced > 0 {
            tracing::info!(count = synced, "Synced pending writes to MinIO");
        }
        
        Ok(())
    }
}
```


---

## 6. Configuration

### 6.1 Failover Configuration

```toml
[failover]
enabled = true

[failover.postgres]
# Circuit breaker settings
circuit_breaker_enabled = true
failure_threshold = 5
circuit_timeout_seconds = 30
half_open_max_calls = 3

# Cache settings
redis_cache_enabled = true
cache_ttl_seconds = 300
local_backup_enabled = true
local_backup_path = "/var/lib/vietnam-cron/backup"
backup_interval_seconds = 60

# Health check
health_check_interval_seconds = 5
health_check_timeout_seconds = 3

[failover.minio]
# Circuit breaker settings
circuit_breaker_enabled = true
failure_threshold = 5
circuit_timeout_seconds = 30
half_open_max_calls = 3

# Fallback storage
redis_cache_enabled = true
redis_cache_ttl_seconds = 600
local_disk_enabled = true
local_disk_path = "/var/lib/vietnam-cron/minio-fallback"
max_local_disk_size_gb = 10

# Pending writes
pending_writes_queue_enabled = true
pending_writes_retention_hours = 24
sync_interval_seconds = 30

# Health check
health_check_interval_seconds = 10
health_check_timeout_seconds = 5

[failover.recovery]
# Auto recovery
auto_recovery_enabled = true
recovery_check_interval_seconds = 10
max_recovery_attempts = 3
recovery_backoff_seconds = 60
```

### 6.2 Degraded Mode Behavior Matrix

| Component Down | Scheduler | Worker | API | Data Loss Risk |
|----------------|-----------|--------|-----|----------------|
| **PostgreSQL Primary** | ⚠️ Read-only (cache) | ✅ Continue | ⚠️ Read-only | ❌ None (auto-promote replica) |
| **PostgreSQL All** | ❌ Stopped | ✅ Continue (cached jobs) | ❌ Unavailable | ⚠️ New executions not recorded |
| **MinIO Single Node** | ✅ Normal | ✅ Normal | ✅ Normal | ❌ None (erasure coding) |
| **MinIO All** | ✅ Normal (cached defs) | ⚠️ Limited (no files) | ⚠️ Limited | ⚠️ New files to fallback |
| **PostgreSQL + MinIO** | ❌ Stopped | ⚠️ Cached jobs only | ❌ Unavailable | ⚠️ High (queue in NATS) |
| **Redis** | ⚠️ Slower | ⚠️ Slower | ⚠️ Slower | ❌ None (not primary) |
| **NATS** | ❌ Can't queue | ❌ Can't consume | ✅ Normal | ⚠️ Jobs not queued |

**Legend:**
- ✅ Normal operation
- ⚠️ Degraded mode
- ❌ Stopped/Unavailable


---

## 7. Testing Strategy

### 7.1 Chaos Engineering Tests

```rust
#[cfg(test)]
mod failover_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_postgres_failover() {
        let app = setup_test_app().await;
        
        // 1. Normal operation
        let job = create_test_job(&app).await.unwrap();
        assert!(job.id.is_some());
        
        // 2. Simulate PostgreSQL down
        app.postgres_container.stop().await;
        
        // 3. Should use cache
        let cached_job = app.job_repo.find_by_id(job.id).await.unwrap();
        assert_eq!(cached_job.name, job.name);
        
        // 4. New writes should fail gracefully
        let result = create_test_job(&app).await;
        assert!(result.is_err());
        
        // 5. Restart PostgreSQL
        app.postgres_container.start().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // 6. Should recover automatically
        let new_job = create_test_job(&app).await.unwrap();
        assert!(new_job.id.is_some());
    }
    
    #[tokio::test]
    async fn test_minio_failover() {
        let app = setup_test_app().await;
        
        // 1. Store job definition
        let job_def = create_test_job_definition();
        app.minio_client
            .store_job_definition(job_def.id, &job_def)
            .await
            .unwrap();
        
        // 2. Simulate MinIO down
        app.minio_container.stop().await;
        
        // 3. Should load from cache
        let cached_def = app.minio_client
            .load_job_definition(job_def.id)
            .await
            .unwrap();
        assert_eq!(cached_def.name, job_def.name);
        
        // 4. New writes should go to fallback
        let new_def = create_test_job_definition();
        let result = app.minio_client
            .store_job_definition(new_def.id, &new_def)
            .await;
        assert!(result.is_ok());
        
        // 5. Restart MinIO
        app.minio_container.start().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // 6. Pending writes should sync
        tokio::time::sleep(Duration::from_secs(35)).await;
        let synced_def = app.minio_client
            .load_job_definition(new_def.id)
            .await
            .unwrap();
        assert_eq!(synced_def.name, new_def.name);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_secs(5),
            half_open_max_calls: 2,
        });
        
        // 1. Closed state - failures increment
        for _ in 0..2 {
            let result = cb.call(async { Err(anyhow!("fail")) }).await;
            assert!(result.is_err());
        }
        assert!(matches!(cb.state(), CircuitState::Closed));
        
        // 2. Third failure opens circuit
        let result = cb.call(async { Err(anyhow!("fail")) }).await;
        assert!(result.is_err());
        assert!(matches!(cb.state(), CircuitState::Open));
        
        // 3. Requests rejected while open
        let result = cb.call(async { Ok(()) }).await;
        assert!(result.is_err());
        
        // 4. Wait for timeout
        tokio::time::sleep(Duration::from_secs(6)).await;
        
        // 5. Transitions to half-open
        let result = cb.call(async { Ok(()) }).await;
        assert!(result.is_ok());
        assert!(matches!(cb.state(), CircuitState::HalfOpen));
        
        // 6. Success closes circuit
        let result = cb.call(async { Ok(()) }).await;
        assert!(result.is_ok());
        assert!(matches!(cb.state(), CircuitState::Closed));
    }
}
```

### 7.2 Load Testing with Failures

```bash
#!/bin/bash
# Chaos test script

echo "Starting chaos test..."

# Start load test
k6 run --vus 100 --duration 10m load-test.js &
LOAD_TEST_PID=$!

# Inject failures
sleep 2m
echo "Stopping PostgreSQL primary..."
kubectl delete pod postgres-primary-0

sleep 1m
echo "Stopping MinIO node..."
kubectl delete pod minio-0

sleep 2m
echo "Recovering services..."
kubectl rollout restart statefulset postgres
kubectl rollout restart statefulset minio

# Wait for load test to complete
wait $LOAD_TEST_PID

echo "Chaos test completed"
```


---

## 8. Operational Runbook

### 8.1 PostgreSQL Failure Response

**Symptoms:**
- `postgres_health_status` metric = 0
- Alert: "PostgreSQLDown"
- Logs: "PostgreSQL health check failed"

**Immediate Actions:**
1. Check PostgreSQL cluster status:
   ```bash
   kubectl get pods -l app=postgres
   patronictl list
   ```

2. Check if automatic failover occurred:
   ```bash
   kubectl logs postgres-primary-0 | grep "promoted"
   ```

3. Verify application is using cache:
   ```bash
   curl http://api:8080/health | jq '.postgres'
   # Should show: fallback_active: true
   ```

4. Monitor pending sync queue:
   ```bash
   curl http://api:9090/metrics | grep pending_sync_queue_size
   ```

**Recovery Steps:**
1. If primary down, wait for auto-promotion (30-60 seconds)
2. If all instances down, restore from backup
3. Once recovered, verify sync:
   ```bash
   kubectl logs scheduler-0 | grep "Replayed execution updates"
   ```

### 8.2 MinIO Failure Response

**Symptoms:**
- `minio_health_status` metric = 0
- Alert: "MinIODown"
- Logs: "MinIO unavailable"

**Immediate Actions:**
1. Check MinIO cluster status:
   ```bash
   kubectl get pods -l app=minio
   mc admin info myminio
   ```

2. Check disk space:
   ```bash
   kubectl exec minio-0 -- df -h /data
   ```

3. Verify fallback storage:
   ```bash
   curl http://api:8080/health | jq '.minio'
   # Should show: fallback_active: true
   ```

4. Check local disk usage:
   ```bash
   kubectl exec worker-0 -- du -sh /var/lib/vietnam-cron/minio-fallback
   ```

**Recovery Steps:**
1. If single node down, MinIO auto-heals (erasure coding)
2. If multiple nodes down, restart pods:
   ```bash
   kubectl rollout restart statefulset minio
   ```
3. Monitor sync progress:
   ```bash
   kubectl logs worker-0 | grep "Synced pending writes"
   ```

### 8.3 Both PostgreSQL + MinIO Down (Disaster)

**Symptoms:**
- Both health metrics = 0
- System in critical degraded mode
- Only cached jobs executing

**Immediate Actions:**
1. **DO NOT PANIC** - Jobs in queue still executing
2. Check NATS queue depth:
   ```bash
   nats stream info jobs
   ```
3. Verify workers still processing:
   ```bash
   kubectl logs worker-0 | grep "Job execution"
   ```

**Recovery Priority:**
1. **PostgreSQL first** (critical for new scheduling)
2. **MinIO second** (files can wait)
3. **Verify data sync** after both recovered

**Post-Recovery:**
1. Check pending sync queues cleared
2. Verify no data loss
3. Review incident and update runbook


---

## 9. Summary & Recommendations

### 9.1 Failover Capabilities Summary

| Failure Scenario | RTO (Recovery Time) | RPO (Data Loss) | Degraded Mode | Auto Recovery |
|------------------|---------------------|-----------------|---------------|---------------|
| PostgreSQL Primary Down | < 1 minute | 0 | Read-only API | ✅ Yes (Patroni) |
| PostgreSQL All Down | N/A | < 1 minute | Cached jobs only | ✅ Yes (replay queue) |
| MinIO Single Node | 0 (transparent) | 0 | None | ✅ Yes (erasure coding) |
| MinIO All Down | N/A | 0 | Local fallback | ✅ Yes (sync queue) |
| Redis Down | 0 (degraded perf) | 0 | Slower queries | ⚠️ Manual restart |
| NATS Down | N/A | 0 | No new jobs | ⚠️ Manual restart |

### 9.2 Best Practices

**Infrastructure:**
1. ✅ Deploy PostgreSQL với Patroni/Stolon (3+ nodes)
2. ✅ Deploy MinIO distributed mode (4+ nodes)
3. ✅ Use PgBouncer cho connection pooling
4. ✅ Redis Sentinel hoặc Cluster (3+ nodes)
5. ✅ NATS JetStream cluster (3+ nodes)

**Application:**
1. ✅ Implement circuit breakers cho tất cả external calls
2. ✅ Cache aggressively (write-through, read-through)
3. ✅ Queue pending writes trong NATS với retention
4. ✅ Local disk backup cho critical data
5. ✅ Health checks mỗi 5-10 giây

**Monitoring:**
1. ✅ Alert khi circuit breaker open
2. ✅ Alert khi fallback storage active > 5 phút
3. ✅ Alert khi pending sync queue > 100
4. ✅ Dashboard hiển thị degraded mode status
5. ✅ Track RTO/RPO metrics

**Testing:**
1. ✅ Chaos engineering tests hàng tuần
2. ✅ Disaster recovery drills hàng tháng
3. ✅ Load testing với failure injection
4. ✅ Verify auto-recovery mechanisms
5. ✅ Test data consistency after recovery

### 9.3 Trade-offs & Limitations

**Advantages:**
- ✅ Zero downtime cho single node failures
- ✅ Graceful degradation thay vì crash
- ✅ No data loss với proper configuration
- ✅ Auto recovery không cần manual intervention
- ✅ Observable với metrics và alerts

**Limitations:**
- ⚠️ Degraded mode có reduced functionality
- ⚠️ Cache có thể stale (TTL-based)
- ⚠️ Local disk fallback không share giữa workers
- ⚠️ Large files (>100MB) không thể process khi MinIO down
- ⚠️ Eventual consistency khi sync pending writes

**Cost:**
- 💰 Cần thêm infrastructure (replicas, cache, disk)
- 💰 Complexity tăng (circuit breakers, sync logic)
- 💰 Monitoring và alerting overhead
- 💰 Testing và maintenance effort

### 9.4 Next Steps

1. **Phase 1: Basic Resilience** (Week 1-2)
   - Implement circuit breakers
   - Add health checks
   - Setup Redis cache

2. **Phase 2: PostgreSQL HA** (Week 3-4)
   - Deploy Patroni cluster
   - Implement cache fallback
   - Add pending writes queue

3. **Phase 3: MinIO HA** (Week 5-6)
   - Deploy MinIO distributed
   - Implement local disk fallback
   - Add sync mechanisms

4. **Phase 4: Testing & Validation** (Week 7-8)
   - Chaos engineering tests
   - Load testing with failures
   - Disaster recovery drills
   - Documentation và runbooks

---

## Kết Luận

Với chiến lược failover này, hệ thống Vietnam Enterprise Cron có thể:

1. **Survive single node failures** mà không downtime
2. **Degrade gracefully** khi multiple nodes down
3. **Auto-recover** khi services khả dụng trở lại
4. **Preserve data** với pending writes queue
5. **Observable** với comprehensive metrics và alerts

Hệ thống sẽ **không bao giờ hoàn toàn down** miễn là có ít nhất:
- 1 Worker node (để execute cached jobs)
- 1 NATS node (để queue jobs)
- Redis hoặc local disk (để cache)

**Trade-off chính**: Tăng complexity và cost để đạt được high availability và zero data loss.

