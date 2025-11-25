---
inclusion: always
---
# RECC 2025 – Rust Enterprise Code of Conduct
## Bộ quy tắc bắt buộc 100% khi dùng AI sinh code Rust (Kiro, Cursor, Copilot, Cody...)

### 1. Nguyên tắc chung – Không khoan nhượng
1. Không bao giờ dùng `unwrap()` / `expect()` trong production code → chỉ được dùng trong `main` khi crash là hợp lý hoặc trong test
2. Luôn dùng `?` + custom error với `#[derive(thiserror::Error)]`
3. Mọi async function phải có `#[tracing::instrument(skip(...))]` 
4. Không dùng `tokio::spawn` nếu có thể dùng queue (NATS/Redis Streams)
5. Graceful shutdown bắt buộc trên SIGTERM & Ctrl+C
6. Không dùng `println!` → chỉ dùng `tracing::info!/warn!/error!`
7. Không dùng `Arc<Mutex<T>>` nếu có thể thay bằng channel hoặc broadcast

### 2. Cấu trúc project chuẩn doanh nghiệp Việt Nam (không được sai dù chỉ 1 file)
src/
├── main.rs              → ≤ 100 dòng, chỉ wiring
├── bin/
│   ├── scheduler.rs
│   └── worker.rs
├── config.rs            → config crate + layered (default → file → env)
├── errors.rs            → thiserror + #[from] mọi external error
├── models/              → struct + FromRow + Serialize + Deserialize
├── api/                 → routes + middleware
├── scheduler/           → engine + distributed lock
├── worker/              → executor trait + implementations
├── queue/               → NATS JetStream hoặc Redis Streams
├── telemetry.rs         → tracing + opentelemetry + prometheus
├── db/                  → migrations + compile-time queries
└── web/                 → HTMX templates (nếu có dashboard)

### 3. 20 quy tắc “one-liner” AI phải nhớ thuộc lòng

```rust
// 1. Mọi struct public
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]

// 2. Mọi handler & async fn
#[tracing::instrument(skip(state, pool, redis))]

// 3. DB query – ưu tiên compile-time
sqlx::query_as!(Job, "SELECT * FROM job WHERE id = $1", id)

// 4. Redis distributed lock → RedLock 3+ node
let lock = redlock_rs::RedLock::new(clients);

// 5. Spawn task phải có error handling
tokio::spawn(async move {
    if let Err(e) = process().await {
        tracing::error!(error = %e, "Task failed");
    }
});

// 6. Không clone String vô tội vạ
fn log(name: &str)

// 7. Health check trả ngay 200
get(|| async { "OK" })

// 8. Metrics endpoint
route("/metrics", get(prometheus_handler))

// 9. Timeout mọi external call
reqwest::Client::builder().timeout(Duration::from_secs(30))

// 10. Retry phải có jitter
backoff::ExponentialBackoffBuilder::new().with_jitter().build()

// 11. Idempotency key bắt buộc
let execution_id = Uuid::new_v4();

// 12. Không commit secret → chỉ có .env.example

// 13. Config layered bắt buộc
Config::builder()
    .add_source(File::with_name("config/default"))
    .add_source(File::with_name("config/local").required(false))
    .add_source(Environment::with_prefix("APP"))
    .build()?

// 14. Mọi error phải derive thiserror
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("DB error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
}

// 15. Graceful shutdown mẫu
tokio::signal::ctrl_c().await.ok();

// 16. File naming – tên file phải mô tả rõ nội dung
// ✅ user_authentication.rs, job_scheduler.rs, minio_storage.rs
// ❌ utils.rs, helpers.rs, common.rs

// 17. File size limit – tối đa 300-400 dòng
// Nếu vượt quá → tách thành module con với mod.rs

// 18. Module organization – tách theo responsibility
// api/handlers/jobs.rs          → job CRUD
// api/handlers/executions.rs    → execution history
// api/handlers/import_export.rs → import/export logic

// 19. Searchability – tên file = keyword search
// Tìm "minio" → minio.rs, minio_client.rs
// Tìm "auth" → auth.rs, authentication.rs, auth_middleware.rs

// 20. Module split strategy khi file quá dài:
// Before: scheduler.rs (800 lines)
// After:  scheduler/
//         ├── mod.rs           → public API
//         ├── trigger.rs       → trigger detection
//         ├── lock.rs          → distributed locking
//         └── publisher.rs     → queue publishing


---

## Phần bổ sung: Chi tiết quy tắc 16-20 (File Naming & Module Organization)

### A. Nguyên tắc đặt tên file – Bắt buộc 100%

#### A.1. Tên file phải mô tả rõ ràng nội dung
**Mục đích**: Tìm kiếm nhanh bằng file name, không cần mở file để biết nội dung

**✅ ĐÚNG:**
- `user_authentication.rs` → biết ngay là xử lý authentication
- `job_scheduler.rs` → biết ngay là scheduler logic
- `minio_storage.rs` → biết ngay là MinIO integration
- `webhook_validator.rs` → biết ngay là validate webhooks
- `distributed_lock.rs` → biết ngay là distributed locking

**❌ SAI:**
- `utils.rs` → không biết chứa gì
- `helpers.rs` → quá generic
- `common.rs` → không rõ ràng
- `misc.rs` → tên tệ nhất
- `lib.rs` (ngoại trừ library root) → không mô tả

#### A.2. Keyword search strategy
Khi cần tìm code liên quan đến một chức năng, search bằng file name:

| Keyword | Expected Files |
|---------|---------------|
| `minio` | `minio.rs`, `minio_client.rs`, `minio_config.rs` |
| `auth` | `auth.rs`, `authentication.rs`, `auth_middleware.rs` |
| `webhook` | `webhook.rs`, `webhook_handler.rs`, `webhook_validator.rs` |
| `redis` | `redis.rs`, `redis_lock.rs`, `redis_client.rs` |
| `job` | `job.rs`, `job_scheduler.rs`, `job_executor.rs` |

### B. Giới hạn độ dài file – Tối đa 300-400 dòng

#### B.1. Tại sao giới hạn?
1. **Tìm kiếm nhanh** → ít hơn 400 dòng = scan nhanh
2. **Dễ review** → không phải scroll quá nhiều
3. **Single Responsibility** → file nhỏ = responsibility rõ ràng
4. **Maintainability** → dễ refactor, dễ test

#### B.2. Cách đếm dòng
```bash
# Đếm dòng code (không tính comments và blank lines)
tokei src/
```

**Ngưỡng cảnh báo:**
- 300-400 dòng: Cân nhắc tách
- 400-500 dòng: Nên tách ngay
- >500 dòng: BẮT BUỘC tách

### C. Chiến lược tách module – Khi nào và như thế nào

#### C.1. Khi nào cần tách?

**Tín hiệu 1: File quá dài (>400 dòng)**
```rust
// src/scheduler.rs (800 lines) → PHẢI TÁCH
```

**Tín hiệu 2: Nhiều responsibilities trong 1 file**
```rust
// src/api/handlers.rs chứa:
// - Job CRUD
// - Execution queries
// - Variable management
// - Auth logic
// → PHẢI TÁCH thành handlers/jobs.rs, handlers/executions.rs, etc.
```

**Tín hiệu 3: Khó tìm kiếm function**
```rust
// Phải Ctrl+F nhiều lần mới tìm thấy function → PHẢI TÁCH
```

#### C.2. Cách tách module đúng chuẩn

**Bước 1: Xác định responsibilities**
```
scheduler.rs chứa:
1. Trigger detection logic
2. Distributed locking
3. Queue publishing
4. Configuration
```

**Bước 2: Tạo folder + mod.rs**
```bash
mkdir src/scheduler
touch src/scheduler/mod.rs
```

**Bước 3: Tách từng responsibility thành file riêng**
```
src/scheduler/
├── mod.rs           → public API, re-exports
├── trigger.rs       → trigger detection logic (200 lines)
├── lock.rs          → distributed locking (150 lines)
├── publisher.rs     → queue publishing (180 lines)
└── config.rs        → configuration (100 lines)
```

**Bước 4: Setup mod.rs**
```rust
// src/scheduler/mod.rs
mod trigger;
mod lock;
mod publisher;
mod config;

pub use trigger::ScheduleTrigger;
pub use lock::DistributedLock;
pub use publisher::JobPublisher;
pub use config::SchedulerConfig;

// Shared constants
pub const DEFAULT_POLL_INTERVAL: u64 = 5;
pub const MAX_LOCK_TTL: u64 = 30;
```

### D. Ví dụ thực tế từ Vietnam Enterprise Cron

#### D.1. API Handlers (Before & After)

**❌ BEFORE: Tất cả trong 1 file**
```
src/api/handlers.rs (1200 lines)
- create_job()
- get_job()
- update_job()
- delete_job()
- list_executions()
- get_execution()
- create_variable()
- get_variable()
- login()
- logout()
- import_job()
- export_job()
```

**✅ AFTER: Tách theo responsibility**
```
src/api/handlers/
├── mod.rs              (50 lines)   → router setup
├── jobs.rs             (250 lines)  → job CRUD
├── executions.rs       (180 lines)  → execution history
├── variables.rs        (200 lines)  → variable management
├── auth.rs             (150 lines)  → authentication
└── import_export.rs    (220 lines)  → import/export logic
```

**Lợi ích:**
- Tìm job CRUD → mở `jobs.rs`
- Tìm auth logic → mở `auth.rs`
- Tìm import/export → mở `import_export.rs`
- Không cần scroll 1200 dòng!

#### D.2. Worker Executor (Before & After)

**❌ BEFORE: Tất cả executors trong 1 file**
```
src/worker/executor.rs (800 lines)
- ExecutorTrait
- HttpExecutor
- DatabaseExecutor
- SftpExecutor
- FileProcessingExecutor
```

**✅ AFTER: Tách theo job type**
```
src/worker/executor/
├── mod.rs              (80 lines)   → ExecutorTrait definition
├── http.rs             (250 lines)  → HTTP job executor
├── database.rs         (280 lines)  → Database job executor
├── sftp.rs             (200 lines)  → SFTP job executor
└── file_processing.rs  (220 lines)  → File processing executor
```

### E. Quy tắc mod.rs – Chỉ làm "gatekeeper"

#### E.1. mod.rs CHỈ chứa:
1. Module declarations (`mod xxx;`)
2. Public re-exports (`pub use xxx::*;`)
3. Shared types/constants
4. Tối đa 50-100 dòng

#### E.2. mod.rs KHÔNG chứa:
1. Business logic
2. Function implementations
3. Complex structs
4. Tests

#### E.3. Ví dụ mod.rs chuẩn
```rust
// src/scheduler/mod.rs (60 lines)

mod trigger;
mod lock;
mod publisher;

pub use trigger::{ScheduleTrigger, TriggerError};
pub use lock::{DistributedLock, LockError};
pub use publisher::{JobPublisher, PublishError};

// Shared constants
pub const DEFAULT_POLL_INTERVAL: u64 = 5;
pub const MAX_LOCK_TTL: u64 = 30;
pub const LOCK_KEY_PREFIX: &str = "scheduler:lock:";

// Shared types
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub poll_interval: u64,
    pub lock_ttl: u64,
}
```

### F. Checklist khi tạo file mới – BẮT BUỘC

Trước khi tạo file mới, kiểm tra:

- [ ] **Tên file mô tả rõ nội dung** (không dùng utils.rs, helpers.rs, common.rs)
- [ ] **Sử dụng snake_case** (không dùng PascalCase hoặc camelCase)
- [ ] **File size ≤ 400 dòng** (nếu dự kiến dài hơn → tách ngay từ đầu)
- [ ] **Chỉ 1 responsibility per file** (không mix nhiều concerns)
- [ ] **Có thể tìm kiếm bằng keyword** trong tên file
- [ ] **Nếu cần tách module** → tạo folder + mod.rs (không để file đơn lẻ quá 400 dòng)

### G. Anti-patterns – TUYỆT ĐỐI TRÁNH

#### ❌ Anti-pattern 1: Generic file names
```
src/utils.rs          → Không biết chứa gì
src/helpers.rs        → Quá generic
src/common.rs         → Không rõ ràng
src/misc.rs           → Tệ nhất
```

#### ❌ Anti-pattern 2: God files (>1000 dòng)
```
src/api/handlers.rs (1500 lines)  → Không thể maintain
src/worker/executor.rs (1200 lines) → Khó tìm kiếm
```

#### ❌ Anti-pattern 3: Business logic trong mod.rs
```rust
// src/scheduler/mod.rs
mod trigger;

// ❌ SAI: Business logic trong mod.rs
pub async fn schedule_job(job: Job) -> Result<()> {
    // 100 lines of logic...
}
```

#### ❌ Anti-pattern 4: Không tách module khi cần
```
src/scheduler.rs (800 lines)
// Lý do không tách: "Tôi lười" → KHÔNG CHẤP NHẬN
```

### H. Tóm tắt – Quy tắc vàng

1. **Tên file = Mô tả nội dung** → Tìm kiếm nhanh
2. **File size ≤ 400 dòng** → Dễ đọc, dễ maintain
3. **1 file = 1 responsibility** → Single Responsibility Principle
4. **Tách module khi cần** → Không để file quá dài
5. **mod.rs = Gatekeeper** → Chỉ re-export, không chứa logic

**Nhớ:** 
> "Tên file tốt = Tiết kiệm 50% thời gian tìm kiếm code"
> "File ngắn = Code dễ review, dễ test, dễ maintain"

