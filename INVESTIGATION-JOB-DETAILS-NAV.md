# Investigation: Navigation Breaks After Job Details

## Symptom
After clicking on a job to view its details at `/dashboard/jobs/{job_id}`, navigation menu (Dashboard, Jobs, Executions, Variables) stops working until the browser is refreshed.

## HTMX Flow Analysis

### 1. Job Link Configuration (from `_job_table.html` line 19-20)
```html
<a href="/dashboard/jobs/{{ job.id }}"
   hx-get="/dashboard/jobs/{{ job.id }}"
   hx-target="#main-content"
   hx-push-url="true">
```

**Analysis:**
- `hx-target="#main-content"` - targets the main content div
- No `hx-swap` attribute specified - **defaults to `innerHTML`**
- `hx-push-url="true"` - updates browser URL

### 2. Navigation Menu Configuration (from `layout.html` lines 352-359)
```html
<a href="/dashboard/jobs"
   hx-get="/dashboard/jobs"
   hx-target="#main-content"
   hx-push-url="true">Jobs</a>
```

**Analysis:**
- Same configuration: targets `#main-content`
- No `hx-swap` specified - **defaults to `innerHTML`**

### 3. Handler Response (from `dashboard.rs` lines 397-403)

```rust
let template = if is_htmx {
    "_job_details_content.html"  // Partial content
} else {
    "job_details.html"           // Full page with layout
};
```

### 4. Current Template Structure (WITHOUT wrapper)

**`_job_details_content.html` (current running version):**
```html
<script>...</script>
<div class="card">
  <!-- Job details -->
</div>
<script>...</script>
```

**Missing:** `<div id="main-content">` wrapper

### 5. Layout Structure (from `layout.html` line 366)
```html
<main>
    <div class="container">
        <div id="main-content">
            {% block content %}{% endblock %}
        </div>
    </div>
</main>
```

## Root Cause Analysis

### When Job Link is Clicked:

**Step 1:** HTMX sends GET to `/dashboard/jobs/{job_id}` with `HX-Request: true` header

**Step 2:** Server responds with `_job_details_content.html` (no wrapper):
```html
<script>console.log('Job details');</script>
<div class="card">...</div>
<script>function exportJob() {...}</script>
```

**Step 3:** HTMX swaps this into `#main-content` using `innerHTML` (default)

**Expected behavior with `innerHTML`:**
```html
<!-- BEFORE swap -->
<div id="main-content">
  <div class="card">Jobs list</div>
</div>

<!-- AFTER swap -->
<div id="main-content">
  <script>console.log('Job details');</script>
  <div class="card">Job details</div>
  <script>function exportJob() {...}</script>
</div>
```

**The `#main-content` div should STILL EXIST** because `innerHTML` only replaces the content INSIDE the target, not the target itself.

## Mystery: Why Does Navigation Break?

### Hypothesis 1: Template Has Wrong Swap Behavior
Let me check if there's an explicit `hx-swap` somewhere that's causing `outerHTML` instead of `innerHTML`.

### Hypothesis 2: Duplicate ID Issue
If `_job_details_content.html` was previously created WITH a `<div id="main-content">` wrapper, we'd have:

```html
<div id="main-content">              <!-- Original from layout -->
  <div id="main-content">             <!-- From response - DUPLICATE! -->
    <script>...</script>
    <div class="card">...</div>
  </div>
</div>
```

This creates duplicate IDs, which is invalid HTML. HTMX may get confused about which element to target.

### Hypothesis 3: Script Execution Order
The scripts in the response might be interfering with HTMX's event handling.

## ROOT CAUSE IDENTIFIED ✅

### The Problem

The **running API binary** was built from an OLD version of the code where `job_details_partial` handler:
- **Does NOT detect HTMX requests** (no `headers: HeaderMap` parameter)
- **Always renders `job_details.html`** (full page with layout), not the partial

### What Happens (Step by Step)

**1. User clicks job name link:**
```html
<a href="/dashboard/jobs/{id}"
   hx-get="/dashboard/jobs/{id}"
   hx-target="#main-content">
```

**2. HTMX sends request with `HX-Request: true` header**

**3. OLD handler ignores HTMX header and renders FULL PAGE:**
```rust
// OLD CODE (git HEAD):
pub async fn job_details_partial(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,  // ❌ No headers parameter!
) -> Result<Html<String>, ErrorResponse> {
    // ...
    let html = TEMPLATES.render("job_details.html", &context)?;  // ❌ Always full page!
    Ok(Html(html))
}
```

**4. Response is a FULL HTML page:**
```html
<!DOCTYPE html>
<html>
<head>...</head>
<body>
    <header>
        <nav>Dashboard | Jobs | Executions | Variables</nav>
    </header>
    <main>
        <div class="container">
            <div id="main-content">
                <div class="card">Job Details...</div>
            </div>
        </div>
    </main>
</body>
</html>
```

**5. HTMX swaps this ENTIRE page into `#main-content`:**
```html
<!-- Current DOM BEFORE swap -->
<body>
    <nav>Dashboard | Jobs | Executions | Variables</nav>
    <main>
        <div id="main-content">
            Jobs list...
        </div>
    </main>
</body>

<!-- DOM AFTER swap (CORRUPTED!) -->
<body>
    <nav>Dashboard | Jobs | Executions | Variables</nav>  ← Original nav
    <main>
        <div id="main-content">  ← Original div
            <!DOCTYPE html>      ← ❌ INVALID! Doctype inside div!
            <html>
            <body>
                <nav>...</nav>   ← ❌ Duplicate nav!
                <div id="main-content">  ← ❌ Nested main-content!
                    <div class="card">Job Details</div>
                </div>
            </body>
            </html>
        </div>
    </main>
</body>
```

**6. Result:**
- ❌ Invalid HTML (doctype/html/body inside a div)
- ❌ Duplicate navigation menu
- ❌ Nested `#main-content` divs
- ❌ Browser DOM corruption
- ❌ HTMX can't find correct target for subsequent clicks
- ✅ Refresh fixes it because full page loads properly

### Why Refresh Fixes It

When you refresh the browser, it requests `/dashboard/jobs/{id}` WITHOUT the `HX-Request` header (normal browser navigation). The handler returns the full page, which loads correctly with proper HTML structure.

### The Fix (Already in Uncommitted Code)

**NEW CODE (uncommitted in working directory):**
```rust
pub async fn job_details_partial(
    State(state): State<AppState>,
    headers: HeaderMap,  // ✅ Added headers parameter
    Path(id): Path<Uuid>,
) -> Result<Html<String>, ErrorResponse> {
    let is_htmx = headers.get("HX-Request").is_some();  // ✅ Detect HTMX

    let template = if is_htmx {
        "_job_details_content.html"  // ✅ Return only content for HTMX
    } else {
        "job_details.html"           // Full page for browser navigation
    };

    let html = TEMPLATES.render(template, &context)?;
    Ok(Html(html))
}
```

**NEW TEMPLATE (`_job_details_content.html` - created):**
```html
<div id="main-content">
    <script>...</script>
    <div class="card">Job Details...</div>
</div>
```

### Resolution Steps Required

1. ✅ Template `_job_details_content.html` created with proper wrapper
2. ⏳ **Rebuild API binary** to include new handler code
3. ⏳ **Restart API service** to load new binary and template
4. ⏳ Test navigation works after viewing job details

### Related Files

- `api/src/handlers/dashboard.rs` - Handler with HTMX detection (uncommitted)
- `api/templates/_job_details_content.html` - Partial template (new file)
- `api/templates/job_details.html` - Full page template (existing)
