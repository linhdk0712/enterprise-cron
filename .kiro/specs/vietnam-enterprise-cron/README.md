# Vietnam Enterprise Cron System - Specification

## Tổng Quan

Đây là specification đầy đủ cho **Vietnam Enterprise Cron System** - một hệ thống distributed job scheduling và execution platform được xây dựng bằng Rust, thay thế các implementation Java Quartz + Spring Batch trong các doanh nghiệp Việt Nam (ngân hàng, viễn thông, thương mại điện tử).

## Cấu Trúc Tài Liệu

### 📋 Core Specification Documents

1. **requirements.md** - Requirements Document
   - 19 requirements với user stories và acceptance criteria
   - Định nghĩa "WHAT" - hệ thống phải làm gì
   - Glossary với tất cả thuật ngữ kỹ thuật

2. **design.md** - Design Document  
   - Architecture và data flow
   - Components, interfaces, và data models
   - 151 correctness properties cho property-based testing
   - Testing strategy và technology stack
   - Định nghĩa "HOW" - hệ thống được xây dựng như thế nào

3. **tasks.md** - Implementation Plan
   - 40 tasks với sub-tasks chi tiết
   - Mỗi task reference đến requirements cụ thể
   - Property-based tests được đánh dấu optional (*)
   - Định nghĩa "WHEN" - thứ tự thực hiện

### 📊 Sequence Diagrams

Thư mục chứa 13 sequence diagrams (PlantUML format):

- `sequence-01-job-scheduling.puml` - Job scheduling flow
- `sequence-02-job-execution.puml` - Job execution flow
- `sequence-03-distributed-locking.puml` - Distributed locking với Redis
- `sequence-04-retry-circuit-breaker.puml` - Retry và circuit breaker
- `sequence-05-authentication-keycloak.puml` - Keycloak authentication
- `sequence-06-authentication-database.puml` - Database authentication
- `sequence-07-webhook-validation.puml` - Webhook signature validation
- `sequence-08-sse-realtime-updates.puml` - Server-Sent Events
- `sequence-09-multi-step-job-execution.puml` - Multi-step jobs với MinIO
- `sequence-10-file-processing-job.puml` - File processing (Excel/CSV)
- `sequence-11-webhook-trigger.puml` - Webhook triggers
- `sequence-12-job-import-export.puml` - Job import/export
- `sequence-13-sftp-job.puml` - SFTP operations

Xem `SEQUENCE-DIAGRAMS-README.md` để biết chi tiết.

### 📚 Supporting Documents

- `DESIGN-UPDATES-NEEDED.md` - Tracking design updates
- `REQUIREMENTS-CHANGES.md` - Tracking requirements changes
- `USE-CASES-README.md` - Use cases documentation
- `DEPENDENCY-UPDATES.md` - Dependency version tracking and update log (NEW)
- `QUICK-START.md` - Quick start guide for developers

## Tính Năng Chính

### Core Features (Requirements 1-12)
- ✅ Distributed job scheduling (Cron, Fixed Delay, Fixed Rate, One-Time)
- ✅ Variable management (Global và Job-specific)
- ✅ Multiple job types (HTTP, Database)
- ✅ Exactly-once execution với idempotency
- ✅ Retry với exponential backoff và circuit breaker
- ✅ Comprehensive observability (Logging, Metrics, Tracing)
- ✅ Real-time HTMX dashboard với SSE
- ✅ High availability và dynamic configuration
- ✅ Flexible authentication (Keycloak hoặc Database)
- ✅ Vietnamese documentation

### Advanced Features (Requirements 13-19)
- ✅ Multi-step jobs với Job Context
- ✅ MinIO storage cho job definitions và execution context
- ✅ Step output references với JSONPath
- ✅ File processing (Excel/CSV) với transformations
- ✅ Webhook triggers với HMAC signature validation
- ✅ Multiple trigger methods (Scheduled, Manual, Webhook)
- ✅ Job import/export với sensitive data masking
- ✅ SFTP operations (Download/Upload)

## Technology Stack

> **Version Policy**: Latest stable versions, quarterly updates  
> **Last Updated**: January 2025  
> **See**: `.kiro/steering/tech.md` for complete list

### Core
- **Rust 1.75+** (2021 Edition) - Type-safe systems programming
- **Tokio 1.35+** - Async runtime
- **Axum 0.7** - Web framework
- **PostgreSQL 14+** - System database (sqlx 0.8)
- **Redis 7.0+** - Distributed locking (RedLock)
- **NATS 2.10+** - Job queue (JetStream)
- **MinIO 2024+** - Object storage (rust-s3 0.34)

### Job Executors
- **reqwest 0.12** - HTTP client (updated from 0.11)
- **sqlx 0.8, mysql_async 0.34, oracle 0.6** - Database drivers
- **calamine 0.24, rust_xlsxwriter 0.65, csv 1.3** - File processing
- **ssh2 0.9** - SFTP operations

### Observability
- **tracing 0.1** - Structured logging
- **metrics-exporter-prometheus 0.15** - Metrics (updated from 0.13)
- **tracing-opentelemetry 0.23** - Distributed tracing (updated from 0.22)

### Testing
- **proptest 1.4** - Property-based testing (100+ iterations)
- **mockall 0.12** - Mocking
- **testcontainers 0.17** - Integration testing (updated from 0.15)

## Quy Trình Làm Việc

### 1️⃣ Trước Khi Bắt Đầu Implementation

**BẮT BUỘC**: Đọc Pre-Implementation Checklist

```
File: .kiro/steering/pre-implementation-checklist.md
```

Checklist này yêu cầu bạn phải:
- ✅ Đọc requirements liên quan
- ✅ Đọc design sections liên quan
- ✅ Xem sequence diagrams liên quan
- ✅ Review steering rules
- ✅ Hiểu đầy đủ task dependencies

**KHÔNG BAO GIỜ** bỏ qua bước này!

### 2️⃣ Trong Quá Trình Implementation

Tuân thủ 100% các quy tắc trong:
```
File: .kiro/steering/implments-rules.md (RECC 2025)
```

Các quy tắc quan trọng:
- ❌ Không dùng `unwrap()` hoặc `expect()` trong production code
- ✅ Luôn dùng `?` operator với custom errors
- ✅ Mọi async function phải có `#[tracing::instrument]`
- ✅ Compile-time query checking với sqlx
- ✅ Graceful shutdown cho SIGTERM/SIGINT
- ✅ Structured logging (JSON format)

### 3️⃣ Sau Khi Hoàn Thành Task

- ✅ Run tests (nếu có)
- ✅ Check diagnostics với `getDiagnostics` tool
- ✅ Verify code match với design document
- ✅ Update task status trong tasks.md
- ✅ Commit với clear message

## Cách Sử Dụng Spec Này

### Cho Developers

1. **Đọc requirements.md** để hiểu nghiệp vụ
2. **Đọc design.md** để hiểu kiến trúc
3. **Xem sequence diagrams** để hiểu flows
4. **Mở tasks.md** và chọn task để implement
5. **Tuân thủ pre-implementation checklist**
6. **Code theo RECC 2025 rules**

### Cho AI Agents (Kiro, Cursor, Copilot)

Khi được yêu cầu implement một task:

1. **PHẢI** đọc các tài liệu liên quan trước:
   ```
   readMultipleFiles([
     "requirements.md",
     "design.md", 
     "sequence-*.puml",
     ".kiro/steering/*.md"
   ])
   ```

2. **PHẢI** phân tích và hiểu đầy đủ trước khi code

3. **PHẢI** giải thích trong response:
   - Đã đọc tài liệu nào
   - Hiểu requirements như thế nào
   - Tại sao implementation match với design

4. **KHÔNG được** bỏ qua bước đọc tài liệu!

### Cho Project Managers

- **requirements.md** - Track feature scope và acceptance criteria
- **tasks.md** - Track implementation progress
- **design.md** - Review architecture decisions
- **Correctness Properties** - Understand quality guarantees

## Correctness Properties

Hệ thống có **151 correctness properties** được định nghĩa trong design.md:

- Properties 1-7: Scheduling
- Properties 8-19: Variable Management  
- Properties 20-28: Job Execution
- Properties 29-38: Reliability
- Properties 39-47: Observability
- Properties 48-54: Dashboard and API
- Properties 55-62: High Availability
- Properties 63-75: Authentication & Authorization
- Properties 76-151: Advanced Features (Multi-step, File Processing, Webhooks, SFTP, Import/Export)

Mỗi property sẽ được implement bằng property-based test với **minimum 100 iterations**.

## Testing Strategy

### Unit Tests
- Test individual components và functions
- Mock external dependencies với mockall
- Co-locate với source files (`.test.rs`)

### Property-Based Tests
- Test universal properties với proptest
- Minimum 100 iterations per property
- Tagged với `// Feature: vietnam-enterprise-cron, Property N: <description>`
- Marked as optional (*) trong tasks.md

### Integration Tests
- Test component interactions
- Use testcontainers cho PostgreSQL, Redis, NATS
- Test end-to-end flows

## Dependency Management

### Update Policy
- **Quarterly Reviews**: Every 3 months, review and update dependencies
- **Security Updates**: Apply immediately when advisories are published
- **Version Strategy**: Use latest stable versions, avoid nightly/beta

### Version Tracking
See `DEPENDENCY-UPDATES.md` for:
- Complete update history
- Migration notes for breaking changes
- Security advisories addressed
- Rollback procedures

### Tools
```bash
# Check for security vulnerabilities
cargo audit

# Check for outdated dependencies
cargo outdated

# Update dependencies
cargo update
```

### Key Updates (January 2025)
- ✅ sqlx 0.7 → 0.8 (performance improvements)
- ✅ reqwest 0.11 → 0.12 (better async support)
- ✅ jsonwebtoken 9.2 → 9.3 (security fixes)
- ✅ testcontainers 0.15 → 0.17 (better Docker support)
- ✅ All observability crates updated to latest

## Deployment

### Docker
- Multi-stage Dockerfile (<50MB final image)
- docker-compose.yml với tất cả services

### Kubernetes
- Helm chart với templates cho tất cả components
- StatefulSets cho PostgreSQL, Redis, NATS, MinIO
- HorizontalPodAutoscaler cho workers

## Documentation

Tất cả documentation được viết bằng **Tiếng Việt** theo Requirements 11.

## Liên Hệ & Support

Nếu có câu hỏi hoặc cần clarification về spec:
1. Kiểm tra tài liệu liên quan trước
2. Xem sequence diagrams
3. Hỏi user để cập nhật spec nếu thiếu thông tin

---

**Lưu ý**: Spec này là living document. Khi có thay đổi requirements hoặc design, cần cập nhật tất cả tài liệu liên quan và sequence diagrams.

**Version**: 1.0  
**Last Updated**: 2025-01-21  
**Status**: Ready for Implementation
