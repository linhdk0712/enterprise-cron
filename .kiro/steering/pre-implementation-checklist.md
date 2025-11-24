---
inclusion: always
---

# Quy Định Kiểm Tra Trước Khi Triển Khai (Pre-Implementation Checklist)

## Nguyên Tắc Bắt Buộc

**QUAN TRỌNG**: Trước khi bắt đầu triển khai BẤT KỲ tính năng mới nào hoặc thực hiện BẤT KỲ task nào trong file `.kiro/specs/vietnam-enterprise-cron/tasks.md`, bạn PHẢI thực hiện đầy đủ các bước kiểm tra sau:

## Checklist Bắt Buộc

### 1. Đọc Requirements Document
```
File: .kiro/specs/vietnam-enterprise-cron/requirements.md
```

**Phải kiểm tra:**
- [ ] Đọc toàn bộ requirements liên quan đến task đang thực hiện
- [ ] Hiểu rõ User Story và mục đích nghiệp vụ
- [ ] Nắm vững TẤT CẢ Acceptance Criteria của requirement
- [ ] Xác định các requirement phụ thuộc (dependencies)
- [ ] Kiểm tra Glossary để hiểu đúng thuật ngữ

**Lý do**: Requirements định nghĩa "WHAT" - hệ thống phải làm gì. Không hiểu requirements = triển khai sai nghiệp vụ.

### 2. Đọc Design Document
```
File: .kiro/specs/vietnam-enterprise-cron/design.md
```

**Phải kiểm tra:**
- [ ] Đọc phần Overview và Key Design Principles
- [ ] Hiểu Architecture và Data Flow liên quan
- [ ] Xem xét Components and Interfaces cần implement
- [ ] Nghiên cứu Data Models và Database Schema
- [ ] Đọc kỹ Correctness Properties liên quan đến task
- [ ] Kiểm tra Testing Strategy cho loại component đang làm
- [ ] Xem Error Handling Strategy
- [ ] Kiểm tra Technology Stack và dependencies cần dùng

**Lý do**: Design định nghĩa "HOW" - hệ thống được xây dựng như thế nào. Không hiểu design = code không nhất quán với kiến trúc.

### 3. Xem Sequence Diagrams
```
Thư mục: .kiro/specs/vietnam-enterprise-cron/
Files: sequence-*.puml và SEQUENCE-DIAGRAMS-README.md
```

**Phải kiểm tra:**
- [ ] Đọc SEQUENCE-DIAGRAMS-README.md để hiểu tổng quan
- [ ] Xem sequence diagram liên quan đến task đang làm
- [ ] Hiểu flow tương tác giữa các components
- [ ] Nắm rõ thứ tự các bước thực thi
- [ ] Xác định các điểm tích hợp (integration points)

**Các sequence diagrams quan trọng:**
- `sequence-01-job-scheduling.puml` - Job scheduling flow
- `sequence-02-job-execution.puml` - Job execution flow  
- `sequence-03-distributed-locking.puml` - Distributed locking
- `sequence-04-retry-circuit-breaker.puml` - Retry và circuit breaker
- `sequence-05-authentication-keycloak.puml` - Keycloak authentication
- `sequence-06-authentication-database.puml` - Database authentication
- `sequence-07-webhook-validation.puml` - Webhook validation
- `sequence-08-sse-realtime-updates.puml` - Server-Sent Events
- `sequence-09-multi-step-job-execution.puml` - Multi-step jobs
- `sequence-10-file-processing-job.puml` - File processing
- `sequence-11-webhook-trigger.puml` - Webhook triggers
- `sequence-12-job-import-export.puml` - Job import/export
- `sequence-13-sftp-job.puml` - SFTP operations

**Lý do**: Sequence diagrams cho thấy "WHEN" và "WHO" - khi nào và ai tương tác với ai. Không hiểu flow = tích hợp sai.

### 4. Đọc Steering Rules
```
Thư mục: .kiro/steering/
```

**Phải kiểm tra:**
- [ ] `tech.md` - Technology stack và dependencies
- [ ] `structure.md` - Project organization và module structure
- [ ] `product.md` - Product overview và capabilities
- [ ] `implments-rules.md` - RECC 2025 coding standards (BẮT BUỘC 100%)

**Lý do**: Steering rules định nghĩa "STANDARDS" - chuẩn mực code và tổ chức dự án. Không tuân thủ = code không maintainable.

### 5. Kiểm Tra Task Dependencies
```
File: .kiro/specs/vietnam-enterprise-cron/tasks.md
```

**Phải kiểm tra:**
- [ ] Xác định task hiện tại trong task list
- [ ] Kiểm tra tất cả parent tasks đã hoàn thành chưa
- [ ] Kiểm tra tất cả prerequisite tasks đã hoàn thành chưa
- [ ] Đọc kỹ task description và sub-bullets
- [ ] Xác định requirements được reference trong task
- [ ] Kiểm tra xem có checkpoint tasks nào cần chạy không

**Lý do**: Tasks có dependencies. Làm sai thứ tự = phải làm lại.

## Quy Trình Thực Hiện Task

### Bước 1: Pre-Implementation Review (TRƯỚC KHI CODE)
```
1. Đọc requirements liên quan (5-10 phút)
2. Đọc design sections liên quan (10-15 phút)
3. Xem sequence diagrams liên quan (5 phút)
4. Review steering rules (5 phút)
5. Xác nhận hiểu đầy đủ yêu cầu
```

### Bước 2: Implementation (KHI CODE)
```
1. Tuân thủ 100% RECC 2025 rules (implments-rules.md)
2. Follow design patterns trong design.md
3. Implement theo đúng interfaces đã định nghĩa
4. Sử dụng đúng technology stack trong tech.md
5. Organize code theo structure.md
6. Viết code với correctness properties trong đầu
```

### Bước 3: Verification (SAU KHI CODE)
```
1. Kiểm tra code tuân thủ RECC 2025 rules
2. Verify code match với design document
3. Ensure code satisfy acceptance criteria
4. Run tests (nếu có)
5. Check diagnostics với getDiagnostics tool
6. Update task status khi hoàn thành
```

## Các Lỗi Thường Gặp Khi KHÔNG Tuân Thủ

### ❌ Không Đọc Requirements
- Implement sai nghiệp vụ
- Miss acceptance criteria
- Không hiểu use case thực tế
- Code không giải quyết đúng vấn đề

### ❌ Không Đọc Design
- Code không nhất quán với architecture
- Duplicate code hoặc reinvent the wheel
- Sử dụng sai patterns
- Không integrate đúng với existing components

### ❌ Không Xem Sequence Diagrams
- Tương tác giữa components sai
- Thứ tự thực thi sai
- Miss critical steps trong flow
- Integration bugs

### ❌ Không Tuân Thủ Steering Rules
- Code style không consistent
- Sử dụng sai dependencies
- Project structure lộn xộn
- Không maintainable

## Câu Hỏi Tự Kiểm Tra Trước Khi Code

Trả lời "CÓ" cho TẤT CẢ các câu hỏi sau trước khi bắt đầu code:

1. ✅ Tôi đã đọc và hiểu requirements liên quan?
2. ✅ Tôi đã đọc design document sections liên quan?
3. ✅ Tôi đã xem sequence diagrams liên quan?
4. ✅ Tôi đã review steering rules?
5. ✅ Tôi biết task này implement requirement nào?
6. ✅ Tôi biết correctness properties nào cần satisfy?
7. ✅ Tôi biết interfaces nào cần implement?
8. ✅ Tôi biết data models nào cần sử dụng?
9. ✅ Tôi biết dependencies nào cần import?
10. ✅ Tôi biết code nên đặt ở module nào?

Nếu có BẤT KỲ câu trả lời "KHÔNG" nào → DỪNG LẠI và đọc tài liệu trước!

## Ví Dụ: Thực Hiện Task 25.1 (MinIO Integration)

### ✅ Cách Làm ĐÚNG:

```
1. Đọc Requirements 13.2, 13.3, 13.7 về MinIO storage
2. Đọc Design:
   - MinIO trong Storage Layer architecture
   - MinIOService trait interface
   - Path formats: jobs/{job_id}/definition.json
   - Technology: rust-s3 crate
3. Xem sequence-09-multi-step-job-execution.puml
   - Hiểu khi nào load/store từ MinIO
4. Check tech.md:
   - rust-s3 = "0.33"
   - Configuration: [minio] section
5. Check structure.md:
   - Code đặt ở src/storage/minio.rs
6. Check implments-rules.md:
   - Không dùng unwrap()
   - Dùng #[tracing::instrument]
   - Error handling với thiserror
7. BẮT ĐẦU CODE với đầy đủ context
```

### ❌ Cách Làm SAI:

```
1. Đọc task description
2. Google "rust minio"
3. Copy-paste code example
4. Không biết path format phải như thế nào
5. Không biết error handling như thế nào
6. Code không match với design
7. BUG và phải refactor lại
```

## Kết Luận

**QUY TẮC VÀNG**: 
> "Đọc tài liệu 30 phút trước khi code = Tiết kiệm 3 giờ debug sau này"

**KHÔNG BAO GIỜ** bỏ qua bước đọc tài liệu. Đây là quy định BẮT BUỘC, không phải optional.

Nếu bạn thấy tài liệu thiếu hoặc không rõ ràng, hãy hỏi user để cập nhật tài liệu, KHÔNG tự ý đoán và implement.

---

**Lưu ý cho AI Agents (Kiro, Cursor, Copilot, etc.):**

Khi được yêu cầu implement một task, bạn PHẢI:
1. Sử dụng `readFile` hoặc `readMultipleFiles` để đọc các tài liệu liên quan
2. Phân tích và hiểu đầy đủ requirements, design, và sequence diagrams
3. Chỉ sau đó mới bắt đầu viết code
4. Trong response, nêu rõ bạn đã đọc tài liệu nào và hiểu như thế nào
5. Giải thích tại sao implementation của bạn match với requirements và design

**KHÔNG được** bỏ qua bước đọc tài liệu và đi thẳng vào code!
