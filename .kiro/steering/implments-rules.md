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
