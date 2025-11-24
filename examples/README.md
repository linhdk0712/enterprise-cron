# Job Definition Examples

Thư mục này chứa các ví dụ về job definitions cho Vietnam Enterprise Cron System.

## Danh Sách Examples

### 1. HTTP Jobs

#### `job-http-simple.json`
- **Mô tả**: Job HTTP đơn giản với GET request
- **Trigger**: Scheduled (cron)
- **Đặc điểm**: 
  - HTTP GET request với authentication
  - Variable substitution trong headers
  - Basic error handling

### 2. Database Jobs

#### `job-database-query.json`
- **Mô tả**: Job thực thi database query
- **Trigger**: Scheduled (fixed rate)
- **Đặc điểm**:
  - PostgreSQL query execution
  - Parameterized queries
  - Variable substitution trong connection string

### 3. Multi-Step Jobs

#### `job-http-database-multistep.json` ⭐ **RECOMMENDED FOR BEGINNERS**
- **Mô tả**: Ví dụ đơn giản về HTTP + Database pipeline
- **Trigger**: Scheduled (fixed rate) + Manual
- **Đặc điểm**:
  - 5 steps: Fetch → Validate → Save → Count → Notify
  - Step output references rõ ràng
  - Best practices cho beginners
  - Comprehensive comments

#### `job-multi-step.json`
- **Mô tả**: Job với nhiều bước: API → Validation → Database → Notification
- **Trigger**: Scheduled (fixed rate) + Manual
- **Đặc điểm**:
  - 4 steps liên tiếp
  - Step output references: `{{steps.step_id.response.body}}`
  - HTTP và Database operations
  - Error handling với retry

#### `job-complex-workflow.json` ⭐ **COMPREHENSIVE EXAMPLE**
- **Mô tả**: Quy trình ETL phức tạp với tất cả loại job types
- **Trigger**: Scheduled (cron) + Manual + Webhook
- **Đặc điểm**:
  - 9 steps: HTTP → SFTP → File Processing → Database → Notification
  - Kết hợp tất cả job types
  - Webhook trigger với custom payload
  - Variable và step output references
  - Comprehensive error handling

### 4. File Processing Jobs

#### `job-file-processing.json`
- **Mô tả**: Đọc Excel file, xử lý dữ liệu, lưu database, tạo CSV summary
- **Trigger**: Scheduled (cron daily)
- **Đặc điểm**:
  - Excel file reading với sheet selection
  - Data transformations (column mapping, type conversion, filtering)
  - Database insertion
  - CSV file generation

#### `job-csv-processing-pipeline.json` ⭐ **CSV PROCESSING EXAMPLE**
- **Mô tả**: CSV processing pipeline với comprehensive transformations
- **Trigger**: Scheduled (cron daily) + Manual
- **Đặc điểm**:
  - CSV reading với configurable delimiter
  - Multiple data transformations
  - Database validation và insertion
  - Output to both Excel và CSV
  - Email notification với file references

### 5. SFTP Jobs

#### `job-sftp-download.json`
- **Mô tả**: Download files từ SFTP, xử lý CSV, lưu database, upload summary
- **Trigger**: Scheduled (cron daily) + Manual
- **Đặc điểm**:
  - SFTP download với wildcard patterns (`TXN_*.csv`)
  - Password authentication
  - CSV processing với transformations
  - Database insertion
  - Excel report generation
  - SFTP upload

#### `job-sftp-upload.json`
- **Mô tả**: Tạo báo cáo từ database và upload lên SFTP
- **Trigger**: Scheduled (cron nightly) + Manual
- **Đặc điểm**:
  - Database query
  - Excel report generation
  - SFTP upload với SSH key authentication
  - Directory creation
  - Upload logging

#### `job-sftp-bidirectional.json` ⭐ **SFTP COMPREHENSIVE EXAMPLE**
- **Mô tả**: Bidirectional SFTP transfer với file processing
- **Trigger**: Scheduled (cron nightly) + Manual
- **Đặc điểm**:
  - Download với wildcard patterns
  - Excel file processing
  - API validation
  - Database operations
  - Upload to multiple SFTP servers (password + SSH key auth)
  - Both Excel và CSV output formats

### 6. Webhook-Triggered Jobs

#### `job-webhook-trigger.json`
- **Mô tả**: Xử lý payment notification từ webhook
- **Trigger**: Webhook only (+ Manual for testing)
- **Đặc điểm**:
  - Webhook payload access: `{{webhook.payload.field}}`
  - HMAC-SHA256 signature validation
  - Rate limiting (100 requests/minute)
  - Payment validation via API
  - Database update
  - Email notification

#### `job-webhook-advanced.json` ⭐ **WEBHOOK COMPREHENSIVE EXAMPLE**
- **Mô tả**: Advanced webhook handler cho e-commerce order processing
- **Trigger**: Webhook only (+ Manual for testing)
- **Đặc điểm**:
  - Webhook logging cho audit trail
  - Payment validation với external gateway
  - Multiple database operations
  - Invoice generation
  - Fulfillment triggering
  - Multi-channel notifications (Email + Slack)
  - Comprehensive webhook data access examples
  - Rate limiting: 200 requests/minute
  - allow_concurrent=true for high volume

## Cấu Trúc Job Definition

```json
{
    "name": "Job Name",
    "description": "Job description in Vietnamese",
    "schedule": {
        "type": "cron|fixed_rate|fixed_delay|one_time",
        "expression": "0 0 1 * * *",  // For cron
        "timezone": "Asia/Ho_Chi_Minh"
    },
    "triggers": {
        "scheduled": true,
        "manual": true,
        "webhook": {
            "enabled": true,
            "secret_key": "your-secret",
            "rate_limit": {
                "max_requests": 100,
                "window_seconds": 60
            }
        }
    },
    "steps": [
        {
            "id": "step_id",
            "name": "Step Name",
            "type": "http|database|file_processing|sftp",
            "config": {
                // Step-specific configuration
            }
        }
    ],
    "timeout_seconds": 300,
    "max_retries": 3,
    "allow_concurrent": false,
    "enabled": true
}
```

## Job Types

### HTTP Request
```json
{
    "type": "http",
    "config": {
        "method": "GET|POST|PUT",
        "url": "https://api.example.com/endpoint",
        "headers": {
            "Authorization": "Bearer ${API_TOKEN}"
        },
        "body": "{\"key\": \"value\"}",
        "auth": {
            "type": "basic|bearer|oauth2",
            "username": "user",
            "password": "pass"
        },
        "timeout_seconds": 30
    }
}
```

### Database Query
```json
{
    "type": "database",
    "config": {
        "database_type": "postgresql|mysql|oracle",
        "connection_string": "${DB_CONNECTION_STRING}",
        "query": "SELECT * FROM table WHERE id = $1",
        "parameters": ["value"],
        "query_type": "raw_sql|stored_procedure",
        "timeout_seconds": 60
    }
}
```

### File Processing
```json
{
    "type": "file_processing",
    "config": {
        "operation": "read|write",
        "format": "excel|csv",
        "source_path": "path/to/file.xlsx",
        "destination_path": "path/to/output.csv",
        "options": {
            "sheet_name": "Sheet1",
            "delimiter": ",",
            "transformations": [
                {
                    "type": "column_mapping",
                    "from": "Old Name",
                    "to": "new_name"
                },
                {
                    "type": "type_conversion",
                    "column": "amount",
                    "target_type": "decimal"
                },
                {
                    "type": "filter",
                    "condition": "amount > 0"
                }
            ]
        }
    }
}
```

### SFTP Operations
```json
{
    "type": "sftp",
    "config": {
        "operation": "download|upload",
        "host": "sftp.example.com",
        "port": 22,
        "auth": {
            "type": "password|ssh_key",
            "username": "${SFTP_USERNAME}",
            "password": "${SFTP_PASSWORD}",
            "private_key_path": "/secrets/key"
        },
        "remote_path": "/path/to/file.csv",
        "local_path": "{{steps.step1.output.files[0].path}}",
        "options": {
            "wildcard_pattern": "*.csv",
            "recursive": false,
            "create_directories": true,
            "verify_host_key": true,
            "streaming": true
        },
        "timeout_seconds": 300
    }
}
```

## Variable References

### Global và Job-Specific Variables
```
${VARIABLE_NAME}
```

### Webhook Data
```
{{webhook.payload.field}}
{{webhook.query_params.param}}
{{webhook.headers.header_name}}
```

### Step Outputs
```
{{steps.step_id.response.body}}
{{steps.step_id.output.data}}
{{steps.step_id.output.rows[0].column}}
{{steps.step_id.output.files[0].path}}
```

### System Variables
```
{{execution_id}}
{{execution_date}}
{{job_id}}
```

## Schedule Types

### Cron Expression (Quartz Syntax)
```json
{
    "type": "cron",
    "expression": "0 30 7 * * MON-FRI",  // 7:30 AM weekdays
    "timezone": "Asia/Ho_Chi_Minh",
    "end_date": "2025-12-31T23:59:59+07:00"
}
```

### Fixed Rate
```json
{
    "type": "fixed_rate",
    "interval_seconds": 300  // Every 5 minutes
}
```

### Fixed Delay
```json
{
    "type": "fixed_delay",
    "delay_seconds": 600  // 10 minutes after completion
}
```

### One Time
```json
{
    "type": "one_time",
    "execute_at": "2025-01-15T10:00:00+07:00"
}
```

## Trigger Methods

### Scheduled Trigger
- Job tự động chạy theo schedule configuration
- Requires `triggers.scheduled = true`

### Manual Trigger
- User trigger từ dashboard hoặc API
- Requires `triggers.manual = true`
- Endpoint: `POST /api/jobs/{job_id}/trigger`

### Webhook Trigger
- External system trigger qua HTTP POST
- Requires `triggers.webhook.enabled = true`
- URL format: `https://your-domain.com/api/webhooks/{job_id}`
- Signature: HMAC-SHA256 trong header `X-Webhook-Signature`

## Import/Export Jobs

### Export Job
```bash
# Via API
curl -X POST https://your-domain.com/api/jobs/{job_id}/export \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -o job-export.json

# Via Dashboard
Click "Export" button on job details page
```

### Import Job
```bash
# Via API
curl -X POST https://your-domain.com/api/jobs/import \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d @job-definition.json

# Via Dashboard
Click "Import" button and upload JSON file
```

## Best Practices

1. **Variable Management**
   - Sử dụng global variables cho credentials chung
   - Sử dụng job-specific variables cho config riêng
   - Đánh dấu sensitive variables để tự động mask

2. **Error Handling**
   - Set `max_retries` phù hợp với job type
   - Sử dụng `timeout_seconds` để tránh job chạy mãi
   - Set `allow_concurrent = false` cho jobs không thể chạy song song

3. **Step Design**
   - Chia job thành steps nhỏ, dễ debug
   - Sử dụng step output references để pass data
   - Đặt tên step_id rõ ràng, dễ hiểu

4. **Performance**
   - Sử dụng `streaming = true` cho large files (>100MB)
   - Set timeout phù hợp với data size
   - Tránh load toàn bộ data vào memory

5. **Security**
   - Không hardcode credentials trong job definition
   - Sử dụng variables cho sensitive data
   - Enable webhook signature validation
   - Verify SFTP host keys

## Testing Jobs

### Test với Manual Trigger
```bash
curl -X POST https://your-domain.com/api/jobs/{job_id}/trigger \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### Test Webhook
```bash
# Generate HMAC signature
SIGNATURE=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)

# Send webhook
curl -X POST https://your-domain.com/api/webhooks/{job_id} \
  -H "Content-Type: application/json" \
  -H "X-Webhook-Signature: $SIGNATURE" \
  -d "$PAYLOAD"
```

### Monitor Execution
```bash
# Get execution details
curl https://your-domain.com/api/executions/{execution_id} \
  -H "Authorization: Bearer YOUR_TOKEN"

# View Job Context in MinIO
# Path: jobs/{job_id}/executions/{execution_id}/context.json
```

## Troubleshooting

### Job không chạy
- Kiểm tra `enabled = true`
- Kiểm tra `triggers.scheduled = true` nếu dùng schedule
- Xem logs của Scheduler component

### Step fails
- Xem execution details để biết step nào fail
- Check Job Context để xem data từ previous steps
- Verify variable values
- Check timeout settings

### Webhook không trigger
- Verify webhook signature
- Check rate limits
- Ensure job is enabled
- Verify webhook URL format

### File processing errors
- Check file format (Excel vs CSV)
- Verify sheet name for Excel files
- Check delimiter for CSV files
- Ensure file size < streaming threshold

### SFTP connection fails
- Verify host, port, username, password/key
- Check network connectivity
- Verify host key if verification enabled
- Check SFTP server logs

## Liên Hệ & Hỗ Trợ

- Documentation: `/README.md`
- API Documentation: `https://your-domain.com/api/docs`
- Dashboard: `https://your-domain.com/dashboard`
