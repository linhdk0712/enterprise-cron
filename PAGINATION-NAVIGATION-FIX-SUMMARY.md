# Pagination & Navigation Fix Summary

## Vấn đề ban đầu

### 1. Không có phân trang
- Tất cả danh sách (Jobs, Executions, Variables) hiển thị toàn bộ dữ liệu
- Không có cách để navigate qua nhiều trang
- Performance kém khi có nhiều records

### 2. Navigation menu bị stuck sau khi phân trang
- Sau khi click nút phân trang (Next/Previous), không thể click menu items
- Phải refresh trang mới chuyển được menu
- Nguyên nhân: Template content không có wrapper `<div id="main-content">`

## Giải pháp đã triển khai

### A. Backend Changes (api/src/handlers/dashboard.rs)

#### 1. Jobs Pagination
```rust
pub async fn jobs_partial(
    Query(params): Query<ExecutionQueryParams>,
) -> Result<Html<String>, ErrorResponse> {
    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);
    let page = (offset / limit) + 1;
    
    // Get total count
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
        .fetch_one(state.db_pool.pool())
        .await
        .unwrap_or(0);
    
    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i64;
    
    // Fetch with pagination
    let query = format!(
        "SELECT * FROM jobs ORDER BY created_at DESC LIMIT {} OFFSET {}",
        limit, offset
    );
    
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("total_count", &total_count);
}
```

#### 2. Executions Pagination
- Limit: 20 items/page (giảm từ 50)
- Giữ filters: `job_id`, `status`
- Tính total_count với WHERE clause
- Thêm `is_embedded` flag để detect khi load trong job details

#### 3. Variables Pagination
- Limit: 20 items/page
- Fetch từ database thay vì placeholder
- Tính total_count và total_pages

### B. Frontend Changes

#### 1. Template Structure Fix

**Before (SAI):**
```html
<!-- _jobs_content.html -->
<script>...</script>
<div class="card">
  <!-- Content -->
</div>
```

**After (ĐÚNG):**
```html
<!-- _jobs_content.html -->
<div id="main-content">
  <script>...</script>
  <div class="card">
    <!-- Content -->
  </div>
</div>
```

**Áp dụng cho:**
- `_jobs_content.html`
- `_variables_content.html`
- `_dashboard_content.html`
- `_executions_content.html` (có điều kiện với `is_embedded`)

#### 2. Pagination UI Component

**Cấu trúc:**
```html
{% if total_pages > 1 %}
<div style="display: flex; justify-content: space-between; ...">
  <div>
    Showing {{ items | length }} of {{ total_count }} items (Page {{ page }} of {{ total_pages }})
  </div>
  <div style="display: flex; gap: 0.5rem;">
    <!-- First button -->
    <button hx-get="...?offset=0&limit={{ limit }}" 
            hx-target="#main-content" 
            hx-swap="innerHTML">
      « First
    </button>
    
    <!-- Previous button -->
    <button hx-get="...?offset={{ (page - 2) * limit }}&limit={{ limit }}"
            hx-target="#main-content" 
            hx-swap="innerHTML">
      ‹ Previous
    </button>
    
    <!-- Page indicator -->
    <span>{{ page }} / {{ total_pages }}</span>
    
    <!-- Next button -->
    <button hx-get="...?offset={{ page * limit }}&limit={{ limit }}"
            hx-target="#main-content" 
            hx-swap="innerHTML">
      Next ›
    </button>
    
    <!-- Last button -->
    <button hx-get="...?offset={{ (total_pages - 1) * limit }}&limit={{ limit }}"
            hx-target="#main-content" 
            hx-swap="innerHTML">
      Last »
    </button>
  </div>
</div>
{% endif %}
```

**Tính năng:**
- Hiển thị thông tin: "Showing X of Y items (Page N of M)"
- 4 nút navigation: First, Previous, Next, Last
- Disable nút khi ở trang đầu/cuối
- Giữ filters khi chuyển trang

#### 3. Embedded Executions trong Job Details

**Vấn đề:** Khi phân trang trong job details, không được replace toàn bộ `#main-content`

**Giải pháp:**
```rust
// Backend
let is_embedded = params.job_id.is_some();
context.insert("is_embedded", &is_embedded);
```

```html
<!-- Template -->
{% if not is_embedded %}
<div id="main-content">
{% endif %}
  <!-- Content -->
  
  {% set target = "closest .card" if is_embedded else "#main-content" %}
  <button hx-target="{{ target }}" ...>
  
{% if not is_embedded %}
</div>
{% endif %}
```

## Kết quả

### ✅ Đã hoàn thành

1. **Phân trang truyền thống**
   - Jobs: 20 items/page
   - Executions: 20 items/page
   - Variables: 20 items/page
   - UI: First | Previous | N/M | Next | Last

2. **Navigation menu hoạt động**
   - Có thể chuyển menu sau khi phân trang
   - Không cần refresh trang
   - Browser history hoạt động đúng

3. **Giữ filters khi phân trang**
   - Executions: Giữ `status` và `job_id`
   - URL parameters được preserve

4. **Embedded pagination**
   - Job details → Recent Executions có phân trang riêng
   - Không ảnh hưởng đến job details content

### 📊 Performance Improvements

**Before:**
- Load tất cả records (có thể 100+)
- Slow query, large response
- Poor UX với nhiều data

**After:**
- Load 20 records/page
- Fast query với LIMIT/OFFSET
- Better UX, smooth navigation

### 🔍 Testing Checklist

Xem file: `NAVIGATION-TEST-CHECKLIST.md`

## Technical Details

### HTMX Attributes

**Navigation Links:**
```html
<a href="/dashboard/jobs" 
   hx-get="/dashboard/jobs" 
   hx-target="#main-content" 
   hx-push-url="true">
```

**Pagination Buttons:**
```html
<button hx-get="/dashboard/jobs?offset=20&limit=20"
        hx-target="#main-content" 
        hx-swap="innerHTML">
```

**Embedded Context:**
```html
<button hx-get="/dashboard/executions?job_id=xxx&offset=20"
        hx-target="closest .card" 
        hx-swap="innerHTML">
```

### SQL Queries

**Count Query:**
```sql
SELECT COUNT(*) FROM jobs
```

**Paginated Query:**
```sql
SELECT * FROM jobs 
ORDER BY created_at DESC 
LIMIT 20 OFFSET 0
```

**With Filters:**
```sql
SELECT COUNT(*) 
FROM job_executions je
WHERE je.created_at >= NOW() - INTERVAL '30 days'
  AND je.status = 'success'
  AND je.job_id = 'xxx'
```

## Files Changed

### Backend
- `api/src/handlers/dashboard.rs`
  - `jobs_partial()` - Added pagination
  - `executions_partial()` - Added pagination + is_embedded
  - `variables_partial()` - Added pagination

### Frontend Templates
- `api/templates/_jobs_content.html` - Added wrapper + pagination UI
- `api/templates/_executions_content.html` - Added wrapper + pagination UI + embedded logic
- `api/templates/_variables_content.html` - Added wrapper + pagination UI
- `api/templates/_dashboard_content.html` - Added wrapper
- `api/templates/partials/_job_table.html` - Added pagination UI
- `api/templates/job_details.html` - Fixed embedded executions target

### Documentation
- `NAVIGATION-TEST-CHECKLIST.md` - Test cases
- `PAGINATION-NAVIGATION-FIX-SUMMARY.md` - This file

## Lessons Learned

1. **HTMX Target Consistency**: Khi swap innerHTML, phải đảm bảo target element vẫn tồn tại sau swap
2. **Template Structure**: Content templates phải có wrapper với ID để HTMX có thể target
3. **Context-Aware Pagination**: Phân trang cần biết context (standalone vs embedded) để target đúng
4. **Filter Preservation**: Phải pass filters qua URL parameters để giữ state khi phân trang

## Future Improvements

1. **Page Size Selection**: Cho phép user chọn 10/20/50/100 items per page
2. **Jump to Page**: Input box để nhảy trực tiếp đến trang N
3. **Infinite Scroll**: Option để load more thay vì pagination
4. **Cache**: Cache pagination results để improve performance
5. **URL State**: Sync pagination state với URL query params
