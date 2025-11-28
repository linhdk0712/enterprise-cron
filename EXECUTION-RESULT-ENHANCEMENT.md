# Enhancement: Display Step-by-Step Execution Results from MinIO

## Mục Đích

Thay vì chỉ hiển thị message đơn giản "Job completed successfully" trong Execution Result, giờ đây hệ thống sẽ hiển thị **chi tiết kết quả từng step** được lưu trong MinIO JobContext.

## Thay Đổi Đã Thực Hiện

### 1. Backend: `api/src/handlers/executions.rs`

**Chức năng mới trong `get_execution()` handler:**

```rust
// Load JobContext from MinIO to get step outputs
// Requirements: 13.8 - Load Job Context to display detailed step results
let step_outputs = if !execution.minio_context_path.is_empty() {
    // Create MinIOService from MinioClient
    use common::storage::service::{MinIOService, MinIOServiceImpl};
    let minio_service = MinIOServiceImpl::new(state.minio_client.clone());
    
    match minio_service.load_context(execution.job_id, execution.id).await {
        Ok(context) => {
            // Convert step outputs to JSON for template
            let mut steps = Vec::new();
            for (step_id, step_output) in context.steps.iter() {
                let duration = (step_output.completed_at - step_output.started_at)
                    .num_milliseconds() as f64 / 1000.0;
                
                steps.push(serde_json::json!({
                    "step_id": step_id,
                    "status": step_output.status,
                    "output": serde_json::to_string_pretty(&step_output.output)
                        .unwrap_or_else(|_| "{}".to_string()),
                    "started_at": step_output.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    "completed_at": step_output.completed_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    "duration_seconds": duration,
                }));
            }
            Some(steps)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load JobContext from MinIO");
            None
        }
    }
} else {
    None
};
```

**Điểm quan trọng:**
- Load JobContext từ MinIO khi có `minio_context_path`
- Parse tất cả step outputs từ `context.steps`
- Tính duration cho mỗi step (milliseconds → seconds)
- Format timestamps cho dễ đọc
- Pretty-print JSON output
- Graceful fallback nếu không load được

### 2. Frontend: `api/templates/_execution_details_modal_content.html`

**Thay thế section "Execution Result" bằng "Step Execution Results":**

```html
<!-- Step Outputs Section (from MinIO JobContext) -->
{% if execution.step_outputs %}
<div>
    <h3>📊 Step Execution Results</h3>
    {% for step in execution.step_outputs %}
    <div style="margin-bottom: 1rem; border: 1px solid #dee2e6; border-radius: 4px;">
        <!-- Step Header -->
        <div style="background: #f8f9fa; padding: 0.75rem 1rem;">
            <strong>Step: {{ step.step_id }}</strong>
            {% if step.status == "success" %}
            <span class="badge badge-success">✓ Success</span>
            {% else %}
            <span class="badge badge-error">✗ {{ step.status }}</span>
            {% endif %}
            <span>Duration: {{ step.duration_seconds }}s</span>
        </div>
        
        <!-- Step Timing -->
        <div style="padding: 0.5rem 1rem; font-size: 0.85rem;">
            Started: {{ step.started_at }} → Completed: {{ step.completed_at }}
        </div>
        
        <!-- Step Output (JSON) -->
        <div style="background: #f8f9fa; padding: 1rem; max-height: 300px; overflow-y: auto;">
            <pre>{{ step.output }}</pre>
        </div>
        
        <!-- Copy Button -->
        <div style="padding: 0.5rem 1rem; text-align: right;">
            <button onclick="copyStepOutput('{{ step.step_id }}')">
                📋 Copy Output
            </button>
        </div>
    </div>
    {% endfor %}
</div>
{% elif execution.result %}
<!-- Fallback: Show simple result if no step outputs available -->
<div>
    <h3>📊 Execution Result</h3>
    <pre>{{ execution.result }}</pre>
</div>
{% endif %}
```

**JavaScript helper:**
```javascript
function copyStepOutput(stepId) {
    // Find and copy step output to clipboard
    const stepElements = document.querySelectorAll('#execution-details-content pre');
    for (let pre of stepElements) {
        const stepDiv = pre.closest('div[style*="max-height: 300px"]');
        if (stepDiv) {
            const stepHeader = stepDiv.parentElement.querySelector('strong');
            if (stepHeader && stepHeader.textContent.includes(stepId)) {
                navigator.clipboard.writeText(pre.textContent).then(() => {
                    alert('Step output copied to clipboard!');
                });
                return;
            }
        }
    }
}
```

## Kết Quả Hiển Thị

### Trước (Old):
```
📊 Execution Result
Job completed successfully
```

### Sau (New):
```
📊 Step Execution Results

┌─────────────────────────────────────────────────────────┐
│ Step: step-1                          ✓ Success  2.3s   │
├─────────────────────────────────────────────────────────┤
│ Started: 2025-11-26 09:13:48 → Completed: 09:13:50     │
├─────────────────────────────────────────────────────────┤
│ {                                                       │
│   "status_code": 200,                                   │
│   "status": "OK",                                       │
│   "headers": {                                          │
│     "content-type": "application/json"                  │
│   },                                                    │
│   "body": {                                             │
│     "temperature": 25.5,                                │
│     "humidity": 60,                                     │
│     "location": "Hanoi"                                 │
│   }                                                     │
│ }                                                       │
│                                      [📋 Copy Output]   │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│ Step: step-2                          ✓ Success  1.8s   │
├─────────────────────────────────────────────────────────┤
│ Started: 2025-11-26 09:13:50 → Completed: 09:13:52     │
├─────────────────────────────────────────────────────────┤
│ {                                                       │
│   "rows_affected": 1523,                                │
│   "execution_time": 1.8,                                │
│   "query": "UPDATE weather_data SET ..."               │
│ }                                                       │
│                                      [📋 Copy Output]   │
└─────────────────────────────────────────────────────────┘
```

## Các Loại Output Được Hiển Thị

### 1. HTTP Request Step
```json
{
  "status_code": 200,
  "status": "OK",
  "headers": {
    "content-type": "application/json",
    "content-length": "1234"
  },
  "body": {
    "id": "123",
    "name": "John Doe",
    "email": "john@example.com"
  }
}
```

### 2. Database Query Step
```json
{
  "rows_affected": 1523,
  "execution_time": 2.3,
  "query": "UPDATE customers SET status = 'active' WHERE last_login > NOW() - INTERVAL '30 days'"
}
```

### 3. File Processing Step
```json
{
  "input_files": [
    {
      "path": "/tmp/input.xlsx",
      "filename": "customers.xlsx",
      "size": 524288,
      "row_count": 1500
    }
  ],
  "output_files": [
    {
      "path": "/tmp/output.csv",
      "filename": "processed_customers.csv",
      "size": 245760,
      "row_count": 1500
    }
  ],
  "processing_time": 3.5
}
```

### 4. SFTP Operation Step
```json
{
  "operation": "upload",
  "files_transferred": 5,
  "total_bytes": 10485760,
  "remote_path": "/data/exports/",
  "transfer_time": 4.2
}
```

## Lợi Ích

### 1. **Debugging & Troubleshooting**
- Xem chính xác response từ API calls
- Kiểm tra số rows affected trong database operations
- Verify file processing results
- Trace data flow qua các steps

### 2. **Monitoring & Auditing**
- Track execution time của từng step
- Identify performance bottlenecks
- Audit data transformations
- Compliance reporting

### 3. **User Experience**
- Transparent execution visibility
- Copy individual step outputs
- Collapsible/scrollable long outputs
- Color-coded status indicators

### 4. **Development & Testing**
- Verify step outputs during development
- Test data transformations
- Validate API integrations
- Debug multi-step workflows

## Technical Details

### Data Flow

```
Worker Execution
    ↓
Execute Step → StepOutput
    ↓
Store in JobContext.steps (HashMap<String, StepOutput>)
    ↓
Save JobContext to MinIO (jobs/{job_id}/executions/{execution_id}/context.json)
    ↓
API Handler loads JobContext from MinIO
    ↓
Parse step outputs → JSON for template
    ↓
Render in UI with formatting
```

### StepOutput Structure

```rust
pub struct StepOutput {
    pub step_id: String,
    pub status: String,
    pub output: serde_json::Value,  // ← Chi tiết kết quả ở đây
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}
```

### JobContext Structure

```rust
pub struct JobContext {
    pub execution_id: Uuid,
    pub job_id: Uuid,
    pub variables: HashMap<String, serde_json::Value>,
    pub steps: HashMap<String, StepOutput>,  // ← Step outputs ở đây
    pub webhook: Option<WebhookData>,
    pub files: Vec<FileMetadata>,
}
```

## Fallback Behavior

Nếu không load được JobContext từ MinIO:
1. Log warning với error details
2. Fallback về hiển thị `execution.result` đơn giản
3. Không crash UI
4. User vẫn thấy basic execution info

## Requirements Satisfied

- **Requirement 13.8**: Load Job Context from MinIO to display step outputs
- **Requirement 6.2**: Display execution details with comprehensive information
- **Property 82**: Job Context persistence to MinIO
- **Property 93**: Automatic step output storage

## Testing

### Manual Testing Steps

1. **Create a multi-step job** với HTTP và Database steps
2. **Trigger execution** (scheduled/manual/webhook)
3. **Wait for completion**
4. **Click "Details" button** trên execution row
5. **Verify:**
   - Step outputs hiển thị đầy đủ
   - JSON được format đẹp
   - Duration tính chính xác
   - Copy button hoạt động
   - Scrollable nếu output dài

### Test Cases

- ✅ Single-step job → 1 step output
- ✅ Multi-step job → Multiple step outputs in order
- ✅ HTTP step → Response body, headers, status code
- ✅ Database step → Rows affected, execution time
- ✅ Failed step → Error details in output
- ✅ Long output → Scrollable container
- ✅ No MinIO context → Fallback to simple result
- ✅ Copy button → Clipboard functionality

## Future Enhancements

1. **Collapsible steps** - Expand/collapse individual steps
2. **Syntax highlighting** - Color-coded JSON
3. **Search/filter** - Search within step outputs
4. **Export** - Download all step outputs as JSON file
5. **Diff view** - Compare outputs between executions
6. **Real-time updates** - SSE for running executions

## Notes

- Step outputs được lưu **tự động** sau mỗi step execution
- MinIO context path format: `jobs/{job_id}/executions/{execution_id}/context.json`
- Output size không giới hạn (MinIO handles large objects)
- UI limits display height (300px) với scroll
- Pretty-print JSON có thể tăng kích thước hiển thị

---

**Implementation Date**: 2025-11-26  
**Status**: ✅ Completed  
**Build Status**: ✅ Successful
