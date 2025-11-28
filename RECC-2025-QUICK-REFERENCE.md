# RECC 2025 Quick Reference Card

## 🎯 Core Rules (Bắt buộc 100%)

### 1. File Naming
```
✅ GOOD                          ❌ BAD
postgresql_executor.rs           utils.rs
job_processor.rs                 helpers.rs
dashboard_stats.rs               common.rs
circuit_breaker_manager.rs       misc.rs
```

**Rule**: Tên file = Mô tả nội dung = Keyword search

---

### 2. File Size Limit
```
✅ GOOD: ≤ 400 dòng
⚠️  WARNING: 400-500 dòng (nên tách)
❌ BAD: > 500 dòng (BẮT BUỘC tách)
```

**Rule**: File size ≤ 400 dòng

---

### 3. Single Responsibility
```
✅ GOOD: 1 file = 1 responsibility
❌ BAD: 1 file = nhiều responsibilities
```

**Example**:
```rust
// ✅ GOOD
// postgresql.rs - CHỈ PostgreSQL executor
impl PostgreSQLExecutor {
    pub async fn execute_query(...) { }
}

// ❌ BAD
// database.rs - PostgreSQL + MySQL + Oracle
impl DatabaseExecutor {
    async fn execute_postgresql(...) { }
    async fn execute_mysql(...) { }
    async fn execute_oracle(...) { }
}
```

---

### 4. mod.rs = Gatekeeper Only
```rust
// ✅ GOOD - mod.rs (50 lines)
mod postgresql;
mod mysql;
mod oracle;

pub use postgresql::PostgreSQLExecutor;
pub use mysql::MySQLExecutor;
pub use oracle::OracleExecutor;

// Shared types
pub struct DatabaseConfig { }

// ❌ BAD - mod.rs (500 lines)
pub async fn execute_query(...) {
    // 100 lines of business logic
}
```

**Rule**: mod.rs ≤ 100 dòng, chỉ chứa exports + shared types

---

## 📋 Refactoring Checklist

### Khi nào cần refactor?
- [ ] File > 400 dòng
- [ ] File có nhiều responsibilities
- [ ] Khó tìm kiếm function trong file
- [ ] Tên file không mô tả nội dung

### Các bước refactor:
1. [ ] Commit code hiện tại
2. [ ] Xác định responsibilities
3. [ ] Tạo folder + mod.rs
4. [ ] Tách từng responsibility thành file riêng
5. [ ] Chạy `getDiagnostics`
6. [ ] Verify public APIs không đổi
7. [ ] Xóa file cũ

---

## 🔍 Searchability Test

### Test: Bạn có thể tìm thấy code trong < 5 giây?

```bash
# ✅ GOOD - Tìm ngay
"postgresql" → postgresql.rs
"job processor" → job_processor.rs
"dashboard stats" → dashboard/stats.rs

# ❌ BAD - Phải mở nhiều files
"postgresql" → database.rs (line 100-300)
"job processor" → worker.rs (line 500-700)
"dashboard stats" → handlers.rs (line 200-400)
```

---

## 📊 Module Organization Patterns

### Pattern 1: Tách theo Database Type
```
executor/database/
├── mod.rs          → DatabaseExecutor coordinator
├── postgresql.rs   → PostgreSQL implementation
├── mysql.rs        → MySQL implementation
└── oracle.rs       → Oracle implementation
```

**Khi nào dùng**: Nhiều implementations của cùng 1 interface

---

### Pattern 2: Tách theo Feature
```
handlers/dashboard/
├── mod.rs              → Exports + shared types
├── stats.rs            → Dashboard statistics
├── jobs_list.rs        → Jobs list
├── job_details.rs      → Job details
├── executions_list.rs  → Executions list
└── variables_list.rs   → Variables list
```

**Khi nào dùng**: Mỗi feature độc lập, ít shared logic

---

### Pattern 3: Tách theo Responsibility
```
worker/consumer/
├── mod.rs                      → WorkerJobConsumer
├── job_processor.rs            → Job lifecycle
├── step_executor.rs            → Step execution
└── circuit_breaker_manager.rs  → Circuit breaker
```

**Khi nào dùng**: Nhiều responsibilities trong 1 workflow

---

### Pattern 4: Tách theo File Format
```
executor/file/
├── mod.rs               → FileProcessingExecutor
├── excel.rs             → Excel processor
├── csv.rs               → CSV processor
└── transformations.rs   → Data transformations
```

**Khi nào dùng**: Nhiều formats/protocols cần support

---

## 🚫 Anti-Patterns (TRÁNH)

### 1. Generic File Names
```
❌ utils.rs
❌ helpers.rs
❌ common.rs
❌ misc.rs
```

### 2. God Files
```
❌ handlers.rs (1500 lines)
❌ executor.rs (1200 lines)
❌ worker.rs (1000 lines)
```

### 3. Business Logic trong mod.rs
```rust
❌ // mod.rs
pub async fn process_job(...) {
    // 100 lines of logic
}
```

### 4. Over-splitting
```
❌ dashboard/
   ├── get_stats.rs (20 lines)
   ├── format_stats.rs (15 lines)
   └── render_stats.rs (18 lines)
   
✅ dashboard/
   └── stats.rs (53 lines)
```

---

## 🎓 Examples from Refactoring

### Example 1: dashboard.rs (876 lines) → 7 files

**Before**:
```
api/src/handlers/dashboard.rs (876 lines)
- dashboard_index()
- jobs_partial()
- job_details_modal()
- job_details_partial()
- executions_partial()
- variables_partial()
- job_form_page()
```

**After**:
```
api/src/handlers/dashboard/
├── mod.rs (30 lines)
├── stats.rs (148 lines)
├── jobs_list.rs (158 lines)
├── job_details.rs (211 lines)
├── executions_list.rs (182 lines)
├── variables_list.rs (93 lines)
└── job_form.rs (22 lines)
```

**Result**: ✅ All files < 400 lines, easy to find

---

### Example 2: database.rs (713 lines) → 4 files

**Before**:
```
common/src/executor/database.rs (713 lines)
- PostgreSQL executor
- MySQL executor
- Oracle executor
```

**After**:
```
common/src/executor/database/
├── mod.rs (218 lines)
├── postgresql.rs (164 lines)
├── mysql.rs (161 lines)
└── oracle.rs (193 lines)
```

**Result**: ✅ Easy to maintain each database type

---

## 📈 Metrics to Track

### File Size Distribution
```
✅ Target: 100% files ≤ 400 lines
⚠️  Warning: > 10% files > 300 lines
❌ Critical: Any file > 500 lines
```

### Searchability Score
```
✅ Good: Tìm thấy code trong < 5 giây
⚠️  OK: Tìm thấy code trong 5-15 giây
❌ Bad: Tìm thấy code trong > 15 giây
```

### Module Depth
```
✅ Good: 2-3 levels (src/handlers/dashboard/stats.rs)
⚠️  OK: 4 levels
❌ Bad: > 4 levels (quá sâu)
```

---

## 🛠️ Tools & Commands

### Check file sizes
```bash
find . -name "*.rs" -type f | while read file; do 
    echo "$file: $(wc -l < "$file") lines"
done | sort -t: -k2 -n
```

### Find large files
```bash
find . -name "*.rs" -type f -exec wc -l {} + | awk '$1 > 400' | sort -n
```

### Count files by size
```bash
find . -name "*.rs" -type f -exec wc -l {} + | \
awk '{if($1<=200) small++; else if($1<=400) medium++; else large++} 
     END {print "Small (≤200):", small, "\nMedium (201-400):", medium, "\nLarge (>400):", large}'
```

### Run diagnostics
```bash
# In Kiro IDE
getDiagnostics(["path/to/file.rs"])
```

---

## 💡 Quick Tips

### Tip 1: Tên file = Keyword search
```
Cần tìm PostgreSQL code? → Search "postgresql"
Cần tìm job processor? → Search "job_processor"
Cần tìm dashboard stats? → Search "stats"
```

### Tip 2: File nhỏ = Dễ review
```
200 lines = 5 phút review
400 lines = 10 phút review
800 lines = 30 phút review (quá lâu!)
```

### Tip 3: 1 file = 1 PR
```
✅ PR: "Add PostgreSQL executor" (164 lines)
❌ PR: "Add database executors" (713 lines)
```

### Tip 4: Helper functions gần nơi sử dụng
```rust
// ✅ GOOD
// jobs_list.rs
fn get_schedule_type(...) { }  // Private helper
pub async fn jobs_partial(...) {
    let schedule = get_schedule_type(...);
}

// ❌ BAD
// helpers.rs
pub fn get_schedule_type(...) { }
```

---

## 🎯 Success Criteria

### ✅ Refactoring thành công khi:
- [ ] Tất cả files ≤ 400 dòng
- [ ] Tên file mô tả rõ nội dung
- [ ] 1 file = 1 responsibility
- [ ] mod.rs ≤ 100 dòng
- [ ] Tìm thấy code trong < 5 giây
- [ ] 0 diagnostics errors
- [ ] Public APIs không thay đổi
- [ ] Tests pass

---

## 📚 References

- **Full Guide**: `REFACTORING-BEST-PRACTICES.md`
- **Summary**: `REFACTORING-SUMMARY.md`
- **RECC 2025 Rules**: `.kiro/steering/implments-rules.md`

---

## 🆘 Need Help?

### Q: File của tôi 450 dòng, có cần tách không?
**A**: CÓ. Tách ngay để tránh vượt 500 dòng.

### Q: Tách thành bao nhiêu files?
**A**: Tùy số responsibilities. Mỗi responsibility = 1 file.

### Q: mod.rs của tôi 150 dòng, có sao không?
**A**: Hơi dài. Kiểm tra xem có business logic không? Nếu có → tách ra.

### Q: Tên file nên dài bao nhiêu?
**A**: 2-4 từ, mô tả rõ ràng. VD: `job_processor.rs`, `postgresql_executor.rs`

### Q: Có nên tạo file helpers.rs không?
**A**: KHÔNG. Đặt helpers gần nơi sử dụng (private functions).

---

**Remember**: 
> "Tên file tốt + File ngắn = Code maintainable"

**Golden Rule**:
> "Nếu bạn không thể tìm thấy code trong 5 giây, file structure cần refactor"
