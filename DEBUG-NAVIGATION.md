# Debug Navigation Issue

## Bước 1: Kiểm tra Response từ Server

Mở Chrome DevTools (F12) → Network tab:

1. Click vào job name để xem details
2. Tìm request đến `/dashboard/jobs/{job_id}`
3. Click vào request đó
4. Xem tab "Response"
5. Tìm xem có `<div id="main-content">` không?

**Expected:**
```html
<div id="main-content">
  <script>...</script>
  <div class="card">
    ...
  </div>
</div>
```

**If NOT found:** Server chưa load template mới hoặc handler chưa được compile.

## Bước 2: Kiểm tra Request Headers

Trong cùng request, xem tab "Headers":

**Check:**
- Request Headers có `HX-Request: true` không?
- Response có status 200 không?

## Bước 3: Kiểm tra Console Errors

Mở Console tab, tìm:
- HTMX errors
- "Target not found" errors
- Template rendering errors

## Bước 4: Inspect HTML Structure

Sau khi load job details:

1. Right-click vào page → Inspect
2. Tìm element với id="main-content"
3. Verify nó tồn tại và chứa job details content

**If NOT found:** Template không được render đúng.

## Bước 5: Manual Test với curl

```bash
# Test HTMX request
curl -H "HX-Request: true" http://localhost:8080/dashboard/jobs/bbd0f989-7c13-4c19-b8a6-b258a1abb4da > response.html

# Check if main-content exists
grep "main-content" response.html
```

## Common Issues

### Issue 1: Server chưa restart
**Solution:** Stop và start lại server
```bash
# Stop: Ctrl+C
cargo run --bin api
```

### Issue 2: Template cache
**Solution:** Clear build và rebuild
```bash
cargo clean
cargo build
cargo run --bin api
```

### Issue 3: Handler không detect HTMX
**Check:** Handler có `headers: HeaderMap` parameter không?
```rust
pub async fn job_details_partial(
    State(state): State<AppState>,
    headers: HeaderMap,  // ← Must have this
    Path(id): Path<Uuid>,
)
```

### Issue 4: Template pattern không match
**Check:** File name đúng không?
- ✅ `_job_details_content.html`
- ❌ `_job_details_content.htm`
- ❌ `job_details_content.html` (missing underscore)

## Quick Fix Script

```bash
#!/bin/bash
echo "Stopping server..."
pkill -f "cargo run --bin api"

echo "Cleaning build..."
cargo clean

echo "Rebuilding..."
cargo build --bin api

echo "Starting server..."
cargo run --bin api
```
