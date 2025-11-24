# Quick Start Guide - Vietnam Enterprise Cron System

## 🚀 Bắt Đầu Nhanh

### Bước 1: Đọc Tài Liệu (30 phút)

```bash
# 1. Đọc tổng quan
cat README.md

# 2. Đọc requirements (10 phút)
cat requirements.md

# 3. Đọc design overview (15 phút)  
head -n 200 design.md

# 4. Xem sequence diagrams (5 phút)
cat SEQUENCE-DIAGRAMS-README.md
```

### Bước 2: Setup Environment

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Clone và setup
git clone <repo>
cd vietnam-enterprise-cron
```

### Bước 3: Chọn Task Để Làm

```bash
# Mở tasks.md
cat tasks.md

# Tìm task chưa làm ([ ])
# Ví dụ: Task 1.1 - Initialize Rust project
```

### Bước 4: Pre-Implementation Checklist

**QUAN TRỌNG**: Trước khi code, PHẢI làm checklist này!

```bash
# Đọc checklist
cat .kiro/steering/pre-implementation-checklist.md
```

#### ✅ Checklist Nhanh:

1. **Đọc Requirements**
   ```bash
   # Tìm requirements liên quan đến task
   grep -A 20 "Requirement X" requirements.md
   ```

2. **Đọc Design**
   ```bash
   # Tìm design sections liên quan
   grep -A 30 "Component Name" design.md
   grep -A 20 "Property X:" design.md
   ```

3. **Xem Sequence Diagram**
   ```bash
   # Xem diagram liên quan
   cat sequence-XX-*.puml
   ```

4. **Check Steering Rules**
   ```bash
   # Đọc coding standards
   cat .kiro/steering/implments-rules.md
   cat .kiro/steering/tech.md
   cat .kiro/steering/structure.md
   ```

### Bước 5: Implement Task

#### Template Code Structure

```rust
// src/module/component.rs

use anyhow::Result;
use thiserror::Error;
use tracing::{info, error, instrument};
use uuid::Uuid;

// 1. Define errors với thiserror
#[derive(Error, Debug)]
pub enum ComponentError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
}

// 2. Define traits
#[async_trait::async_trait]
pub trait ComponentService {
    async fn do_something(&self, id: Uuid) -> Result<Data>;
}

// 3. Implement struct
pub struct ComponentServiceImpl {
    pool: sqlx::PgPool,
}

// 4. Implement methods với tracing
#[async_trait::async_trait]
impl ComponentService for ComponentServiceImpl {
    #[instrument(skip(self))]
    async fn do_something(&self, id: Uuid) -> Result<Data> {
        info!("Starting operation for id: {}", id);
        
        // KHÔNG dùng unwrap()!
        let result = sqlx::query_as!(
            Data,
            "SELECT * FROM table WHERE id = $1",
            id
        )
        .fetch_one(&self.pool)
        .await?; // Dùng ? operator
        
        info!("Operation completed successfully");
        Ok(result)
    }
}

// 5. Tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_do_something() {
        // Test code here
    }
}
```

#### RECC 2025 Rules - Nhớ Thuộc Lòng

```rust
// ❌ KHÔNG BAO GIỜ làm thế này:
let value = some_option.unwrap();
let result = some_result.expect("failed");

// ✅ LUÔN LUÔN làm thế này:
let value = some_option.ok_or(Error::NotFound)?;
let result = some_result?;

// ❌ KHÔNG dùng println!
println!("Debug: {}", value);

// ✅ Dùng tracing
tracing::info!("Processing value: {}", value);

// ❌ KHÔNG dùng Arc<Mutex<T>> nếu có thể tránh
let shared = Arc::new(Mutex::new(data));

// ✅ Dùng channels
let (tx, rx) = tokio::sync::mpsc::channel(100);

// ✅ LUÔN có #[instrument]
#[instrument(skip(pool, redis))]
async fn my_function(pool: &PgPool, redis: &Redis) -> Result<()> {
    // ...
}

// ✅ Graceful shutdown
tokio::signal::ctrl_c().await?;
info!("Shutting down gracefully...");
// Complete in-flight work
shutdown_tx.send(()).await?;
```

### Bước 6: Testing

```bash
# Run unit tests
cargo test --lib

# Run specific test
cargo test test_name

# Run property tests (nếu có)
cargo test property_

# Check diagnostics
# (Sử dụng getDiagnostics tool trong Kiro)
```

### Bước 7: Verify Implementation

#### Checklist Verification:

- [ ] Code tuân thủ RECC 2025 rules?
- [ ] Không có `unwrap()` hoặc `expect()`?
- [ ] Tất cả async functions có `#[instrument]`?
- [ ] Error handling đúng với `thiserror`?
- [ ] Code match với design document?
- [ ] Satisfy acceptance criteria?
- [ ] Tests pass (nếu có)?
- [ ] No compiler warnings?

### Bước 8: Update Task Status

```bash
# Mark task as complete trong tasks.md
# Thay đổi [ ] thành [x]
```

## 📚 Tài Liệu Tham Khảo Nhanh

### Requirements Mapping

| Requirement | Feature | Tasks |
|-------------|---------|-------|
| 1 | Job Scheduling | 3.1-3.4, 9.1-9.5 |
| 2 | Variable Management | 6.1-6.4 |
| 3 | Job Execution | 12.1-14.6 |
| 4 | Reliability | 7.1-7.3, 8.1-8.4, 11.1-11.4 |
| 5 | Observability | 16.1-16.5 |
| 6 | Dashboard & API | 19.1-21.4 |
| 7 | High Availability | 7.1-7.3, 9.1-9.5 |
| 8 | Error Handling | 1.3 |
| 9 | Deployment | 22.1-22.4 |
| 10 | Authentication | 17.1-18.3 |
| 11 | Documentation | 23.1-23.2 |
| 12 | Code Organization | 1.1-1.4 |
| 13 | Multi-Step Jobs | 25.1-28.3 |
| 14 | Step References | 27.1-27.3 |
| 15 | File Processing | 30.1-30.6 |
| 16 | Webhooks | 33.1-33.6 |
| 17 | Trigger Methods | 34.1-34.5 |
| 18 | Import/Export | 35.1-35.5 |
| 19 | SFTP | 31.1-31.7 |

### Module Structure Quick Reference

```
src/
├── main.rs              → API server entry (≤100 lines)
├── bin/
│   ├── scheduler.rs     → Scheduler binary
│   └── worker.rs        → Worker binary
├── config/              → Configuration management
├── errors/              → Domain errors (thiserror)
├── models/              → Data models
├── scheduler/           → Scheduling logic
├── worker/              → Job execution
│   ├── context.rs       → Job Context management
│   ├── reference.rs     → Reference resolver
│   └── executor/        → Job executors
│       ├── http.rs      → HTTP executor
│       ├── database.rs  → Database executor
│       ├── file.rs      → File processing
│       └── sftp.rs      → SFTP operations
├── api/                 → REST API & handlers
│   ├── handlers/        → Request handlers
│   └── middleware/      → Auth, RBAC, rate limit
├── db/                  → Database layer
│   ├── migrations/      → SQL migrations
│   └── repositories/    → Data access
├── queue/               → NATS JetStream
├── storage/             → MinIO integration
├── telemetry/           → Logging, metrics, tracing
└── web/                 → HTMX templates
```

### Common Commands

```bash
# Development
cargo run                    # Run API server
cargo run --bin scheduler    # Run scheduler
cargo run --bin worker       # Run worker

# Testing
cargo test                   # All tests
cargo test --lib            # Unit tests only
cargo test property_        # Property tests only

# Database
sqlx migrate run            # Run migrations
sqlx migrate revert         # Revert last migration

# Build
cargo build --release       # Release build
cargo clippy               # Linting
cargo fmt                  # Format code

# Docker
docker-compose up -d       # Start all services
docker-compose logs -f     # View logs
```

### Sequence Diagrams Quick Index

| Flow | Diagram | Key Components |
|------|---------|----------------|
| Job Scheduling | sequence-01 | Scheduler, Redis, NATS |
| Job Execution | sequence-02 | Worker, Executor, DB |
| Distributed Lock | sequence-03 | Scheduler, Redis |
| Retry & Circuit Breaker | sequence-04 | Worker, Retry, CB |
| Keycloak Auth | sequence-05 | API, Keycloak |
| Database Auth | sequence-06 | API, DB, JWT |
| Webhook Validation | sequence-07 | API, HMAC |
| SSE Updates | sequence-08 | API, Dashboard |
| Multi-Step Jobs | sequence-09 | Worker, MinIO, Context |
| File Processing | sequence-10 | Worker, MinIO, Files |
| Webhook Trigger | sequence-11 | API, Webhook, Queue |
| Import/Export | sequence-12 | API, MinIO, Mask |
| SFTP Operations | sequence-13 | Worker, SFTP, MinIO |

## 🆘 Troubleshooting

### "Tôi không biết bắt đầu từ đâu?"

→ Đọc README.md và requirements.md trước. Sau đó chọn Task 1.1.

### "Tôi không hiểu requirement này?"

→ Đọc:
1. User Story trong requirements.md
2. Acceptance Criteria chi tiết
3. Glossary để hiểu thuật ngữ
4. Sequence diagram liên quan

### "Code của tôi không compile?"

→ Kiểm tra:
1. Có dùng `unwrap()` không? → Thay bằng `?`
2. Có import đủ dependencies không?
3. Có follow đúng error handling pattern không?
4. Run `cargo clippy` để xem suggestions

### "Test của tôi fail?"

→ Kiểm tra:
1. Code có satisfy acceptance criteria không?
2. Code có match với design document không?
3. Có miss edge cases không?
4. Có đọc correctness property không?

### "Tôi không chắc implementation đúng không?"

→ Tự hỏi:
1. Tôi đã đọc requirements chưa? ✅
2. Tôi đã đọc design chưa? ✅
3. Tôi đã xem sequence diagram chưa? ✅
4. Code match với design không? ✅
5. Satisfy acceptance criteria không? ✅

Nếu tất cả đều ✅ → Implementation đúng!

## 💡 Tips & Best Practices

### Tip 1: Đọc Tài Liệu Trước, Code Sau
- 30 phút đọc = Tiết kiệm 3 giờ debug

### Tip 2: Follow RECC 2025 100%
- Không có exception cho rules này

### Tip 3: Viết Tests Ngay
- Test-driven development giúp catch bugs sớm

### Tip 4: Commit Thường Xuyên
- Mỗi task hoàn thành = 1 commit

### Tip 5: Hỏi Khi Không Rõ
- Đừng đoán, hỏi để cập nhật spec

## 🎯 Success Criteria

Bạn đã làm đúng khi:

✅ Code compile without warnings  
✅ Tests pass (nếu có)  
✅ Tuân thủ 100% RECC 2025 rules  
✅ Match với design document  
✅ Satisfy acceptance criteria  
✅ No `unwrap()` hoặc `expect()`  
✅ All async functions có `#[instrument]`  
✅ Proper error handling với `thiserror`  
✅ Task status updated trong tasks.md  

---

**Remember**: Quality > Speed. Làm đúng từ đầu > Phải sửa sau!

**Happy Coding! 🚀**
