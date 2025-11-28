# Navigation Test Checklist

## Mục đích
Kiểm tra toàn diện navigation menu và phân trang sau khi thêm wrapper `<div id="main-content">`.

## Test Cases

### 1. Navigation Menu - Chuyển trang cơ bản
- [ ] **Dashboard → Jobs**: Click menu Jobs
  - Expected: Hiển thị danh sách jobs
  - Check: URL thay đổi thành `/dashboard/jobs`
  
- [ ] **Jobs → Executions**: Click menu Executions
  - Expected: Hiển thị danh sách executions
  - Check: URL thay đổi thành `/dashboard/executions`
  
- [ ] **Executions → Variables**: Click menu Variables
  - Expected: Hiển thị danh sách variables
  - Check: URL thay đổi thành `/dashboard/variables`
  
- [ ] **Variables → Dashboard**: Click menu Dashboard
  - Expected: Hiển thị dashboard
  - Check: URL thay đổi thành `/dashboard`

### 2. Phân trang Jobs
- [ ] **Jobs page 1 → page 2**: Click "Next ›"
  - Expected: Hiển thị trang 2 của jobs
  - Check: Pagination hiển thị "Page 2 of X"
  
- [ ] **Jobs page 2 → Menu Executions**: Click menu Executions
  - Expected: Chuyển sang executions page 1
  - Check: Navigation hoạt động bình thường
  
- [ ] **Back to Jobs**: Click menu Jobs
  - Expected: Hiển thị jobs page 1 (reset về đầu)
  - Check: Pagination hiển thị "Page 1 of X"

### 3. Phân trang Executions
- [ ] **Executions page 1 → page 2**: Click "Next ›"
  - Expected: Hiển thị trang 2 của executions
  - Check: Pagination hiển thị "Page 2 of X"
  
- [ ] **Executions page 2 → Menu Variables**: Click menu Variables
  - Expected: Chuyển sang variables
  - Check: Navigation hoạt động bình thường
  
- [ ] **Filter + Pagination**: Chọn status filter → Click Next
  - Expected: Giữ filter khi chuyển trang
  - Check: URL có cả `status=` và `offset=`

### 4. Phân trang Variables
- [ ] **Variables page 1 → page 2**: Click "Next ›"
  - Expected: Hiển thị trang 2 của variables
  - Check: Pagination hiển thị "Page 2 of X"
  
- [ ] **Variables page 2 → Menu Dashboard**: Click menu Dashboard
  - Expected: Chuyển về dashboard
  - Check: Navigation hoạt động bình thường

### 5. Job Details - Embedded Executions
- [ ] **Click vào một Job**: Từ Jobs list, click vào job name
  - Expected: Hiển thị job details với Recent Executions
  - Check: URL thay đổi thành `/dashboard/jobs/{job_id}`
  
- [ ] **Pagination trong Job Details**: Click "Next ›" trong Recent Executions
  - Expected: Chỉ phần executions thay đổi, không reload toàn trang
  - Check: Job details vẫn hiển thị ở trên
  
- [ ] **Job Details → Menu Jobs**: Click menu Jobs
  - Expected: Quay về jobs list
  - Check: Navigation hoạt động bình thường

### 6. Links trong Tables
- [ ] **Execution → Job Details**: Click job name trong executions table
  - Expected: Chuyển đến job details
  - Check: URL và content đúng
  
- [ ] **Job Details → Menu Executions**: Click menu Executions
  - Expected: Quay về executions list
  - Check: Navigation hoạt động bình thường

### 7. Browser Navigation
- [ ] **Browser Back**: Click browser back button
  - Expected: Quay về trang trước
  - Check: HTMX push-url hoạt động đúng
  
- [ ] **Browser Forward**: Click browser forward button
  - Expected: Tiến tới trang sau
  - Check: History hoạt động đúng
  
- [ ] **Refresh Page**: F5 hoặc Ctrl+R
  - Expected: Trang reload với đúng URL
  - Check: Content hiển thị đúng

### 8. Edge Cases
- [ ] **Rapid Navigation**: Click nhanh nhiều menu items
  - Expected: Không bị stuck, luôn load được trang
  - Check: Console không có lỗi
  
- [ ] **Pagination + Navigation**: Click Next → Ngay lập tức click menu khác
  - Expected: Navigation hoạt động, không bị conflict
  - Check: Trang đích hiển thị đúng
  
- [ ] **SSE Events**: Khi có SSE event (job created, execution changed)
  - Expected: Chỉ update phần liên quan, không ảnh hưởng navigation
  - Check: Menu vẫn hoạt động bình thường

## Cấu trúc HTML cần kiểm tra

### Layout (layout.html)
```html
<div id="main-content">
  {% block content %}{% endblock %}
</div>
```

### Content Templates (_*_content.html)
```html
<div id="main-content">
  <!-- Content here -->
</div>
```

### Navigation Links
```html
<a href="/dashboard/jobs" 
   hx-get="/dashboard/jobs" 
   hx-target="#main-content" 
   hx-push-url="true">
```

### Pagination Buttons
```html
<button hx-get="/dashboard/jobs?offset=20&limit=20"
        hx-target="#main-content" 
        hx-swap="innerHTML">
```

## Kết quả mong đợi

✅ **PASS**: Tất cả navigation và pagination hoạt động mượt mà
- Menu items luôn clickable
- Phân trang không làm mất navigation
- Browser history hoạt động đúng
- Không cần refresh để chuyển trang

❌ **FAIL**: Nếu bất kỳ điều nào sau xảy ra
- Menu không click được sau khi phân trang
- Phải refresh mới chuyển được menu
- Console có lỗi HTMX
- `#main-content` không tìm thấy

## Debug Tips

Nếu navigation không hoạt động:

1. **Check Console**: Mở DevTools → Console
   - Tìm lỗi HTMX: "htmx:responseError"
   - Tìm lỗi target: "Target not found"

2. **Check HTML Structure**: Inspect element
   - Verify `<div id="main-content">` tồn tại
   - Check nó không bị duplicate
   - Verify nó không bị nested sai

3. **Check HTMX Attributes**:
   - `hx-target="#main-content"` đúng
   - `hx-swap="innerHTML"` đúng
   - `hx-push-url="true"` có trong navigation links

4. **Check Network**: DevTools → Network
   - Verify requests được gửi đúng
   - Check response có chứa `<div id="main-content">`
   - Verify status code 200

## Ghi chú

- Test trên Chrome/Firefox/Safari
- Test cả desktop và mobile view
- Test với network throttling (slow 3G)
- Test với nhiều tabs mở cùng lúc
