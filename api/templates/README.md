# Template Structure Documentation

## ⚠️ CRITICAL: Tera Template Engine Syntax

**Vietnam Enterprise Cron sử dụng Tera template engine, KHÔNG phải Jinja2!**

### ❌ KHÔNG được dùng (Jinja2 syntax):
```jinja2
{% include "file.html" with var=value %}
{% include "file.html" with var1=value1, var2=value2 %}
```

### ✅ Phải dùng (Tera syntax):
```jinja2
{# Option 1: Inline logic (recommended) #}
{% if status == "success" %}
<span class="badge badge-success">Success</span>
{% endif %}

{# Option 2: Include without parameters (variables must be in context) #}
{% include "file.html" %}
```

**Lý do**: Tera không hỗ trợ passing variables qua `with` keyword. Nếu dùng sẽ gây lỗi:
```
expected `%}` or `-%}` or ignore missing mark for include tag
```

## Overview

Template được tổ chức theo pattern **HTMX-friendly** với khả năng render cả full page và partial content.

## Directory Structure

```
api/templates/
├── layout.html                    # Base layout với navbar, styles, scripts
├── partials/                      # Reusable components
│   ├── _empty_state.html         # Empty state component
│   ├── _execution_table.html     # Execution list table
│   ├── _job_import_modal.html    # Job import modal với sensitive data handling
│   ├── _job_table.html           # Job list table
│   ├── _job_type_badge.html      # Job type badge (HTTP, Database, etc.)
│   ├── _modal_styles.html        # Modal CSS styles
│   ├── _stat_card.html           # Dashboard stat card
│   ├── _status_badge.html        # Execution status badge
│   ├── _success_rate_badge.html  # Success rate badge với color coding
│   └── _trigger_source_badge.html # Trigger source badge (Scheduled, Manual, Webhook)
├── _dashboard_content.html        # Dashboard page content
├── _executions_content.html       # Executions page content
├── _jobs_content.html             # Jobs page content
├── _variables_content.html        # Variables page content
├── dashboard.html                 # Dashboard wrapper (HTMX-aware)
├── executions.html                # Executions wrapper (HTMX-aware)
├── jobs.html                      # Jobs wrapper (HTMX-aware)
├── variables.html                 # Variables wrapper (HTMX-aware)
├── job_details.html               # Job details page
└── job_form.html                  # Job creation/edit form
```

## Template Pattern

### HTMX-Aware Wrappers

Mỗi page có 2 modes:

1. **Full Page Mode** (direct navigation): Render với layout.html
2. **Partial Mode** (HTMX request): Chỉ render content

**Example: jobs.html**
```jinja2
{% if is_htmx %}
{# HTMX partial - only content #}
{% include "_jobs_content.html" %}
{% else %}
{# Full page with layout #}
{% extends "layout.html" %}
{% block title %}Jobs - Vietnam Enterprise Cron{% endblock %}
{% block content %}
{% include "_jobs_content.html" %}
{% endblock %}
{% endif %}
```

### Content Templates

Content templates (`_*_content.html`) chứa logic và HTML thực tế:
- Không extend layout
- Có thể include partials
- Được sử dụng bởi cả full page và HTMX requests

### Partials

Partials (`partials/_*.html`) là reusable components:
- **⚠️ QUAN TRỌNG**: Tera KHÔNG hỗ trợ `{% include "file" with var=value %}` syntax (đây là Jinja2)
- Variables phải có sẵn trong context hoặc inline logic trực tiếp
- Không có dependencies
- Có thể nest với nhau

**❌ WRONG - Tera không hỗ trợ:**
```jinja2
{% include "partials/_status_badge.html" with status=execution.status %}
{% include "partials/_job_type_badge.html" with job_type=job.job_type %}
```

**✅ CORRECT - Inline logic hoặc ensure variables in context:**
```jinja2
{# Option 1: Inline logic (recommended for simple cases) #}
{% if execution.status == "running" %}
<span class="badge badge-primary">Running</span>
{% elif execution.status == "success" %}
<span class="badge badge-success">Success</span>
{% endif %}

{# Option 2: Include without parameters (variables must be in context) #}
{% include "partials/_status_badge.html" %}
```

## Handler Pattern

Handlers phải detect HTMX requests và set `is_htmx` flag:

```rust
#[tracing::instrument(skip(state, headers))]
pub async fn jobs_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");
    
    // Detect HTMX request
    let is_htmx = headers.get("HX-Request").is_some();
    context.insert("is_htmx", &is_htmx);
    
    // ... fetch data ...
    
    let html = TEMPLATES
        .render("jobs.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;
    
    Ok(Html(html))
}
```

## Benefits

### 1. No Duplicate Navbar
- HTMX requests chỉ nhận content, không có layout
- Direct navigation nhận full page với navbar

### 2. Code Reusability
- Partials được share giữa nhiều pages
- Badges, tables, modals có thể reuse
- Giảm code duplication

### 3. Maintainability
- Thay đổi badge logic ở 1 nơi → apply cho tất cả
- Dễ test từng component riêng
- Clear separation of concerns

### 4. Performance
- HTMX chỉ load content cần thiết
- Giảm bandwidth
- Faster page transitions

## Naming Conventions

- **Wrappers**: `{page}.html` (e.g., `jobs.html`)
- **Content**: `_{page}_content.html` (e.g., `_jobs_content.html`)
- **Partials**: `partials/_{component}.html` (e.g., `partials/_status_badge.html`)
- **Prefix `_`**: Indicates internal/partial template

## Adding New Pages

1. Create content template: `_newpage_content.html`
2. Create wrapper: `newpage.html` với HTMX pattern
3. Create handler với `is_htmx` detection
4. **Inline repeated logic** instead of creating partials with parameters

## Common Pitfalls & Troubleshooting

### ❌ Error: "expected `%}` or `-%}` or ignore missing mark for include tag"

**Nguyên nhân**: Dùng `with` keyword trong include statement (Jinja2 syntax)

**Sai:**
```jinja2
{% include "partials/_badge.html" with status=execution.status %}
```

**Đúng:**
```jinja2
{# Inline the logic #}
{% if execution.status == "success" %}
<span class="badge badge-success">Success</span>
{% endif %}
```

### ❌ Error: "Failed to parse template"

**Nguyên nhân**: Syntax error trong template

**Checklist:**
- ✅ Tất cả `{% if %}` phải có `{% endif %}`
- ✅ Tất cả `{% for %}` phải có `{% endfor %}`
- ✅ Không dùng `with` keyword
- ✅ String literals dùng quotes: `"value"` hoặc `'value'`
- ✅ Variables không cần quotes: `{{ variable }}`

### ❌ Error: "ERR_CONNECTION_RESET" khi truy cập dashboard

**Nguyên nhân**: Server crash do template parsing error

**Giải pháp:**
1. Check Docker logs: `docker logs vietnam-cron-api --tail 50`
2. Tìm dòng "Failed to parse"
3. Sửa template theo hướng dẫn trên
4. Rebuild Docker image: `docker-compose up -d --build api`

### ✅ Template Validation Checklist

Trước khi commit template mới:

- [ ] Không có `{% include "file" with var=value %}`
- [ ] Tất cả control structures đã đóng đúng
- [ ] Test cả full page và HTMX mode
- [ ] Variables có null checks: `{% if var %}{{ var }}{% endif %}`
- [ ] Inline logic thay vì complex partials

## Best Practices

1. **⚠️ NEVER use `{% include "file" with var=value %}`** - Tera không hỗ trợ!
2. **Prefer inline logic over complex partials** - easier to maintain
3. **Keep content templates focused** - one responsibility
4. **Test both HTMX and direct navigation modes**
5. **Use semantic naming** - clear what component does
6. **Ensure all variables exist in context** before including partials
7. **Add null checks** - use `{% if variable %}` before accessing properties

## Example: Inline Logic vs Partials

### ❌ WRONG - Using 'with' (Jinja2 syntax, not supported by Tera)
```jinja2
{% include "partials/_duration_display.html" with started_at=execution.started_at %}
```

### ✅ CORRECT - Inline Logic (Recommended)
```jinja2
{# Inline duration display logic #}
{% if execution.completed_at and execution.started_at %}
{% set duration_seconds = (execution.completed_at - execution.started_at) | int %}
{% if duration_seconds < 60 %}
<small>{{ duration_seconds }}s</small>
{% elif duration_seconds < 3600 %}
<small>{{ (duration_seconds / 60) | round }}m</small>
{% else %}
<small>{{ (duration_seconds / 3600) | round }}h</small>
{% endif %}
{% elif execution.status == "running" %}
<small class="badge badge-info">Running...</small>
{% else %}
<small>-</small>
{% endif %}
```

### ✅ CORRECT - Include without parameters (if variables already in context)
```jinja2
{# partials/_duration_display.html expects: execution object in context #}
{% include "partials/_duration_display.html" %}
```
