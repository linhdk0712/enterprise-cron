# Nghiên Cứu Phương Án Sử Dụng WebAssembly cho Vietnam Enterprise Cron System

## Tổng Quan

Tài liệu này nghiên cứu các phương án tích hợp WebAssembly (WASM) vào hệ thống Vietnam Enterprise Cron để mở rộng khả năng thực thi job với custom logic an toàn, hiệu năng cao, và đa ngôn ngữ.

## Mục Tiêu Tích Hợp WASM

### 1. Custom Job Logic
- Cho phép user viết custom business logic bằng nhiều ngôn ngữ (Rust, Go, JavaScript, Python, C++)
- Thực thi logic phức tạp không thể biểu diễn bằng HTTP/Database/File processing
- Sandbox an toàn, không thể truy cập tài nguyên hệ thống trái phép

### 2. Data Transformation
- Transform dữ liệu giữa các step với logic phức tạp
- Validate và enrich data từ API responses hoặc database results
- Custom parsing và formatting logic

### 3. Conditional Logic & Routing
- Điều kiện phức tạp để quyết định step tiếp theo
- Dynamic routing dựa trên kết quả step trước
- Business rules engine

### 4. Plugin System
- Mở rộng hệ thống với custom executors
- Third-party integrations không cần rebuild hệ thống
- Marketplace cho WASM modules

## Kiến Trúc Đề Xuất


### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Worker Process                           │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │              Job Execution Engine                       │    │
│  │                                                          │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐ │    │
│  │  │   HTTP   │  │ Database │  │   File   │  │  WASM  │ │    │
│  │  │ Executor │  │ Executor │  │ Executor │  │Executor│ │    │
│  │  └──────────┘  └──────────┘  └──────────┘  └────────┘ │    │
│  │                                                 │        │    │
│  └─────────────────────────────────────────────────┼────────┘    │
│                                                    │             │
│  ┌─────────────────────────────────────────────────▼────────┐    │
│  │            WASM Runtime (Wasmtime)                       │    │
│  │                                                          │    │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐       │    │
│  │  │  Module 1  │  │  Module 2  │  │  Module N  │       │    │
│  │  │  (cached)  │  │  (cached)  │  │  (cached)  │       │    │
│  │  └────────────┘  └────────────┘  └────────────┘       │    │
│  │                                                          │    │
│  │  ┌──────────────────────────────────────────────────┐  │    │
│  │  │         WASI (WebAssembly System Interface)      │  │    │
│  │  │  - File I/O (restricted to MinIO paths)         │  │    │
│  │  │  - Environment variables (job context)          │  │    │
│  │  │  - Clock/Time                                    │  │    │
│  │  │  - Random numbers                                │  │    │
│  │  └──────────────────────────────────────────────────┘  │    │
│  │                                                          │    │
│  │  ┌──────────────────────────────────────────────────┐  │    │
│  │  │         Custom Host Functions                    │  │    │
│  │  │  - log(level, message)                           │  │    │
│  │  │  - get_context(key) -> value                     │  │    │
│  │  │  - set_context(key, value)                       │  │    │
│  │  │  - http_request(config) -> response              │  │    │
│  │  │  - db_query(config) -> result                    │  │    │
│  │  └──────────────────────────────────────────────────┘  │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                  MinIO Storage                           │    │
│  │  - WASM modules: wasm-modules/{module_id}.wasm          │    │
│  │  - Job Context: jobs/{job_id}/executions/{exec_id}/...  │    │
│  └──────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### Component Design

#### 1. WASM Executor

```rust
use wasmtime::*;
use anyhow::Result;

pub struct WasmExecutor {
    engine: Engine,
    module_cache: Arc<RwLock<HashMap<String, Module>>>,
    minio_client: Arc<MinIOService>,
}

impl WasmExecutor {
    pub async fn new(minio_client: Arc<MinIOService>) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
        config.wasm_module_linking(true);
        config.async_support(true);
        
        // Security: Enable resource limits
        config.max_wasm_stack(1024 * 1024); // 1MB stack
        config.consume_fuel(true); // Enable fuel metering
        
        let engine = Engine::new(&config)?;
        
        Ok(Self {
            engine,
            module_cache: Arc::new(RwLock::new(HashMap::new())),
            minio_client,
        })
    }
}

    pub async fn execute(
        &self,
        step: &WasmJobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput> {
        // Load WASM module from MinIO (with caching)
        let module = self.load_module(&step.module_id).await?;
        
        // Create store with fuel limit
        let mut store = Store::new(&self.engine, WasmState::new(context.clone()));
        store.set_fuel(step.fuel_limit.unwrap_or(1_000_000))?;
        store.set_epoch_deadline(step.timeout_seconds.unwrap_or(30));
        
        // Create WASI context with restricted permissions
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .env("JOB_ID", &context.job_id.to_string())?
            .env("EXECUTION_ID", &context.execution_id.to_string())?
            .build();
        
        store.data_mut().wasi = wasi;
        
        // Instantiate module with host functions
        let instance = self.instantiate_with_host_functions(&mut store, &module).await?;
        
        // Call the main function
        let main_func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, &step.function_name)?;
        
        // Prepare input (serialize context to JSON)
        let input_json = serde_json::to_string(&context)?;
        let (input_ptr, input_len) = self.write_to_memory(&mut store, &instance, &input_json)?;
        
        // Execute with timeout
        let result_ptr = tokio::time::timeout(
            Duration::from_secs(step.timeout_seconds.unwrap_or(30)),
            main_func.call_async(&mut store, (input_ptr, input_len))
        ).await??;
        
        // Read result from memory
        let output_json = self.read_from_memory(&mut store, &instance, result_ptr)?;
        let output: serde_json::Value = serde_json::from_str(&output_json)?;
        
        Ok(StepOutput {
            step_id: step.id.clone(),
            status: "success".to_string(),
            output,
            started_at: Utc::now(),
            completed_at: Utc::now(),
        })
    }
    
    async fn load_module(&self, module_id: &str) -> Result<Module> {
        // Check cache first
        {
            let cache = self.module_cache.read().await;
            if let Some(module) = cache.get(module_id) {
                return Ok(module.clone());
            }
        }
        
        // Load from MinIO
        let wasm_bytes = self.minio_client
            .load_file(&format!("wasm-modules/{}.wasm", module_id))
            .await?;
        
        // Compile module
        let module = Module::from_binary(&self.engine, &wasm_bytes)?;
        
        // Cache it
        {
            let mut cache = self.module_cache.write().await;
            cache.insert(module_id.to_string(), module.clone());
        }
        
        Ok(module)
    }
}

#[async_trait]
impl JobExecutor for WasmExecutor {
    async fn execute(&self, step: &JobStep, context: &mut JobContext) -> Result<StepOutput> {
        if let JobType::Wasm(wasm_step) = &step.step_type {
            self.execute(wasm_step, context).await
        } else {
            Err(anyhow!("Invalid step type for WasmExecutor"))
        }
    }
}
```

#### 2. Host Functions Interface

```rust
use wasmtime::*;

struct WasmState {
    context: JobContext,
    wasi: WasiCtx,
}

impl WasmState {
    fn new(context: JobContext) -> Self {
        Self {
            context,
            wasi: WasiCtxBuilder::new().build(),
        }
    }
}

// Host function: log
fn host_log(mut caller: Caller<'_, WasmState>, level: i32, ptr: i32, len: i32) -> Result<()> {
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("failed to find memory export"))?;
    
    let data = memory.data(&caller);
    let message = std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])?;
    
    match level {
        0 => tracing::debug!("{}", message),
        1 => tracing::info!("{}", message),
        2 => tracing::warn!("{}", message),
        3 => tracing::error!("{}", message),
        _ => tracing::info!("{}", message),
    }
    
    Ok(())
}

// Host function: get_context
fn host_get_context(
    mut caller: Caller<'_, WasmState>,
    key_ptr: i32,
    key_len: i32,
) -> Result<i32> {
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("failed to find memory export"))?;
    
    let data = memory.data(&caller);
    let key = std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize])?;
    
    let state = caller.data();
    let value = state.context.variables.get(key)
        .ok_or_else(|| anyhow!("key not found: {}", key))?;
    
    let value_json = serde_json::to_string(value)?;
    
    // Allocate memory in WASM and write value
    let alloc_func = caller.get_export("alloc")
        .and_then(|e| e.into_func())
        .ok_or_else(|| anyhow!("failed to find alloc function"))?;
    
    let result_ptr = alloc_func.typed::<i32, i32>(&caller)?
        .call(&mut caller, value_json.len() as i32)?;
    
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("failed to find memory export"))?;
    
    memory.write(&mut caller, result_ptr as usize, value_json.as_bytes())?;
    
    Ok(result_ptr)
}

// Host function: set_context
fn host_set_context(
    mut caller: Caller<'_, WasmState>,
    key_ptr: i32,
    key_len: i32,
    value_ptr: i32,
    value_len: i32,
) -> Result<()> {
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("failed to find memory export"))?;
    
    let data = memory.data(&caller);
    let key = std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize])?;
    let value_str = std::str::from_utf8(&data[value_ptr as usize..(value_ptr + value_len) as usize])?;
    
    let value: serde_json::Value = serde_json::from_str(value_str)?;
    
    let state = caller.data_mut();
    state.context.variables.insert(key.to_string(), value);
    
    Ok(())
}

// Host function: http_request
async fn host_http_request(
    mut caller: Caller<'_, WasmState>,
    config_ptr: i32,
    config_len: i32,
) -> Result<i32> {
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("failed to find memory export"))?;
    
    let data = memory.data(&caller);
    let config_json = std::str::from_utf8(&data[config_ptr as usize..(config_ptr + config_len) as usize])?;
    
    let config: HttpRequestConfig = serde_json::from_str(config_json)?;
    
    // Execute HTTP request using existing HttpExecutor
    let client = reqwest::Client::new();
    let response = client
        .request(config.method, &config.url)
        .headers(config.headers)
        .body(config.body.unwrap_or_default())
        .send()
        .await?;
    
    let response_json = serde_json::json!({
        "status": response.status().as_u16(),
        "headers": response.headers(),
        "body": response.text().await?,
    });
    
    let response_str = serde_json::to_string(&response_json)?;
    
    // Allocate and write response
    let alloc_func = caller.get_export("alloc")
        .and_then(|e| e.into_func())
        .ok_or_else(|| anyhow!("failed to find alloc function"))?;
    
    let result_ptr = alloc_func.typed::<i32, i32>(&caller)?
        .call(&mut caller, response_str.len() as i32)?;
    
    memory.write(&mut caller, result_ptr as usize, response_str.as_bytes())?;
    
    Ok(result_ptr)
}
```



#### 3. Data Models

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmJobStep {
    pub module_id: String,
    pub function_name: String,
    pub fuel_limit: Option<u64>,
    pub timeout_seconds: Option<u64>,
    pub memory_limit_mb: Option<u32>,
}

// Extend JobType enum
pub enum JobType {
    HttpRequest { /* ... */ },
    DatabaseQuery { /* ... */ },
    FileProcessing { /* ... */ },
    Sftp { /* ... */ },
    Wasm(WasmJobStep),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WasmModule {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub author: String,
    pub minio_path: String,
    pub hash: String, // SHA256 hash for integrity
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### 4. Database Schema

```sql
CREATE TABLE wasm_modules (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    version VARCHAR(50) NOT NULL,
    author VARCHAR(255) NOT NULL,
    minio_path VARCHAR(500) NOT NULL,
    hash VARCHAR(64) NOT NULL, -- SHA256
    size_bytes BIGINT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (name, version)
);

CREATE TABLE wasm_module_permissions (
    id UUID PRIMARY KEY,
    module_id UUID NOT NULL REFERENCES wasm_modules(id) ON DELETE CASCADE,
    permission_type VARCHAR(50) NOT NULL, -- 'http', 'database', 'file', 'network'
    resource_pattern VARCHAR(500), -- e.g., 'https://api.example.com/*'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    INDEX idx_wasm_permissions_module_id (module_id)
);

CREATE TABLE wasm_execution_logs (
    id UUID PRIMARY KEY,
    execution_id UUID NOT NULL REFERENCES job_executions(id) ON DELETE CASCADE,
    module_id UUID NOT NULL REFERENCES wasm_modules(id),
    fuel_consumed BIGINT NOT NULL,
    memory_used_bytes BIGINT NOT NULL,
    duration_ms BIGINT NOT NULL,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    INDEX idx_wasm_logs_execution_id (execution_id),
    INDEX idx_wasm_logs_module_id (module_id)
);
```

## Use Cases

### Use Case 1: Custom Data Transformation

**Scenario**: Transform API response data với business logic phức tạp

**WASM Module (Rust)**:
```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct Context {
    steps: std::collections::HashMap<String, StepOutput>,
}

#[derive(Deserialize)]
struct StepOutput {
    output: Value,
}

#[derive(Serialize)]
struct TransformedData {
    customers: Vec<Customer>,
    total_revenue: f64,
}

#[derive(Serialize)]
struct Customer {
    id: String,
    name: String,
    revenue: f64,
    tier: String,
}

#[no_mangle]
pub extern "C" fn transform_data(input_ptr: *const u8, input_len: usize) -> *const u8 {
    let input = unsafe {
        std::slice::from_raw_parts(input_ptr, input_len)
    };
    
    let context: Context = serde_json::from_slice(input).unwrap();
    
    // Get API response from previous step
    let api_response = &context.steps.get("fetch_customers").unwrap().output;
    let customers_data = api_response["data"]["customers"].as_array().unwrap();
    
    let mut total_revenue = 0.0;
    let mut customers = Vec::new();
    
    for customer in customers_data {
        let revenue = customer["total_orders"].as_f64().unwrap();
        total_revenue += revenue;
        
        // Business logic: Calculate customer tier
        let tier = if revenue > 100000.0 {
            "platinum"
        } else if revenue > 50000.0 {
            "gold"
        } else if revenue > 10000.0 {
            "silver"
        } else {
            "bronze"
        };
        
        customers.push(Customer {
            id: customer["id"].as_str().unwrap().to_string(),
            name: customer["name"].as_str().unwrap().to_string(),
            revenue,
            tier: tier.to_string(),
        });
    }
    
    let result = TransformedData {
        customers,
        total_revenue,
    };
    
    let output = serde_json::to_string(&result).unwrap();
    
    // Return pointer to output
    output.as_ptr()
}
```

**Job Definition**:
```json
{
  "name": "customer-analysis",
  "steps": [
    {
      "id": "fetch_customers",
      "name": "Fetch Customer Data",
      "step_type": {
        "HttpRequest": {
          "method": "GET",
          "url": "https://api.example.com/customers",
          "auth": {
            "Bearer": {
              "token": "{{variables.api_token}}"
            }
          }
        }
      }
    },
    {
      "id": "transform",
      "name": "Transform and Analyze",
      "step_type": {
        "Wasm": {
          "module_id": "customer-transformer-v1",
          "function_name": "transform_data",
          "fuel_limit": 1000000,
          "timeout_seconds": 10
        }
      }
    },
    {
      "id": "save_to_db",
      "name": "Save to Database",
      "step_type": {
        "DatabaseQuery": {
          "database_type": "PostgreSQL",
          "connection_string": "{{variables.db_url}}",
          "query": "INSERT INTO customer_analysis (data) VALUES ($1)",
          "query_type": "RawSql"
        }
      }
    }
  ]
}
```

### Use Case 2: Conditional Routing

**Scenario**: Quyết định step tiếp theo dựa trên kết quả API

**WASM Module (JavaScript via AssemblyScript)**:
```typescript
import { JSON } from "assemblyscript-json";

export function route_decision(input: string): string {
  const context = JSON.parse(input);
  const apiResponse = context.steps.check_inventory.output;
  
  const stockLevel = apiResponse.data.stock_level as i32;
  const threshold = 100;
  
  if (stockLevel < threshold) {
    return JSON.stringify({
      next_step: "send_low_stock_alert",
      priority: "high",
      message: `Stock level ${stockLevel} below threshold ${threshold}`
    });
  } else {
    return JSON.stringify({
      next_step: "continue_normal_flow",
      priority: "normal"
    });
  }
}
```

### Use Case 3: Custom Validation

**Scenario**: Validate dữ liệu với business rules phức tạp

**WASM Module (Go via TinyGo)**:
```go
package main

import (
    "encoding/json"
    "fmt"
)

type ValidationResult struct {
    Valid  bool     `json:"valid"`
    Errors []string `json:"errors"`
}

//export validate_order
func validate_order(inputPtr *byte, inputLen int) *byte {
    input := ptrToString(inputPtr, inputLen)
    
    var context map[string]interface{}
    json.Unmarshal([]byte(input), &context)
    
    order := context["steps"].(map[string]interface{})["fetch_order"].(map[string]interface{})["output"]
    
    result := ValidationResult{Valid: true, Errors: []string{}}
    
    // Business rule 1: Order amount must be positive
    amount := order["amount"].(float64)
    if amount <= 0 {
        result.Valid = false
        result.Errors = append(result.Errors, "Order amount must be positive")
    }
    
    // Business rule 2: Customer must have valid email
    email := order["customer"].(map[string]interface{})["email"].(string)
    if !isValidEmail(email) {
        result.Valid = false
        result.Errors = append(result.Errors, "Invalid customer email")
    }
    
    // Business rule 3: Order items must not exceed limit
    items := order["items"].([]interface{})
    if len(items) > 50 {
        result.Valid = false
        result.Errors = append(result.Errors, "Order cannot have more than 50 items")
    }
    
    output, _ := json.Marshal(result)
    return stringToPtr(string(output))
}

func main() {}
```



## Security Model

### 1. Sandboxing

**Isolation Guarantees**:
- WASM modules chạy trong sandbox, không thể truy cập memory của host
- Không thể gọi system calls trực tiếp
- Chỉ có thể tương tác qua host functions được định nghĩa

**Resource Limits**:
```rust
pub struct WasmSecurityConfig {
    pub max_memory_mb: u32,        // Default: 64MB
    pub max_fuel: u64,             // Default: 1,000,000 instructions
    pub max_execution_time_sec: u64, // Default: 30 seconds
    pub max_stack_size_kb: u32,    // Default: 1MB
}
```

### 2. Permission System

**Permission Types**:
- `http:read` - Cho phép HTTP GET requests
- `http:write` - Cho phép HTTP POST/PUT/DELETE
- `database:read` - Cho phép SELECT queries
- `database:write` - Cho phép INSERT/UPDATE/DELETE
- `file:read` - Cho phép đọc files từ MinIO
- `file:write` - Cho phép ghi files vào MinIO
- `network:external` - Cho phép kết nối external networks

**Permission Enforcement**:
```rust
impl WasmExecutor {
    fn check_permission(
        &self,
        module_id: &str,
        permission: &str,
        resource: &str,
    ) -> Result<()> {
        let permissions = self.load_module_permissions(module_id)?;
        
        for perm in permissions {
            if perm.permission_type == permission {
                if let Some(pattern) = &perm.resource_pattern {
                    if glob_match(pattern, resource) {
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            }
        }
        
        Err(anyhow!("Permission denied: {} for resource {}", permission, resource))
    }
}
```

### 3. Code Signing & Verification

**Module Upload Flow**:
```rust
pub async fn upload_wasm_module(
    module_bytes: Vec<u8>,
    metadata: WasmModuleMetadata,
    signature: String,
) -> Result<WasmModule> {
    // 1. Verify signature
    verify_signature(&module_bytes, &signature, &metadata.author_public_key)?;
    
    // 2. Calculate hash
    let hash = sha256(&module_bytes);
    
    // 3. Validate WASM format
    Module::validate(&engine, &module_bytes)?;
    
    // 4. Static analysis for dangerous patterns
    analyze_wasm_safety(&module_bytes)?;
    
    // 5. Store to MinIO
    let path = format!("wasm-modules/{}.wasm", Uuid::new_v4());
    minio_client.store_file(&path, &module_bytes).await?;
    
    // 6. Save metadata to database
    let module = WasmModule {
        id: Uuid::new_v4(),
        name: metadata.name,
        version: metadata.version,
        minio_path: path,
        hash,
        // ...
    };
    
    db.save_wasm_module(&module).await?;
    
    Ok(module)
}
```

### 4. Audit Logging

**Log All WASM Executions**:
```rust
#[tracing::instrument(skip(self, context))]
async fn execute_wasm(
    &self,
    module_id: &str,
    context: &JobContext,
) -> Result<StepOutput> {
    let start = Utc::now();
    let fuel_before = store.get_fuel()?;
    
    let result = self.execute_internal(module_id, context).await;
    
    let fuel_consumed = fuel_before - store.get_fuel()?;
    let duration = Utc::now() - start;
    
    // Log execution metrics
    self.db.insert_wasm_execution_log(WasmExecutionLog {
        execution_id: context.execution_id,
        module_id: module_id.parse()?,
        fuel_consumed,
        memory_used_bytes: store.data().memory_used(),
        duration_ms: duration.num_milliseconds(),
        error: result.as_ref().err().map(|e| e.to_string()),
        created_at: Utc::now(),
    }).await?;
    
    result
}
```

## Performance Considerations

### 1. Module Caching

**Strategy**:
- Cache compiled modules in memory
- LRU eviction policy
- Pre-compile frequently used modules at startup

```rust
pub struct ModuleCache {
    cache: Arc<RwLock<LruCache<String, Module>>>,
    max_size: usize,
}

impl ModuleCache {
    pub async fn get_or_compile(
        &self,
        module_id: &str,
        engine: &Engine,
        minio: &MinIOService,
    ) -> Result<Module> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(module) = cache.peek(module_id) {
                return Ok(module.clone());
            }
        }
        
        // Load and compile
        let bytes = minio.load_file(&format!("wasm-modules/{}.wasm", module_id)).await?;
        let module = Module::from_binary(engine, &bytes)?;
        
        // Cache it
        {
            let mut cache = self.cache.write().await;
            cache.put(module_id.to_string(), module.clone());
        }
        
        Ok(module)
    }
}
```

### 2. Ahead-of-Time (AOT) Compilation

**Pre-compile modules**:
```rust
pub async fn precompile_module(
    engine: &Engine,
    module_bytes: &[u8],
) -> Result<Vec<u8>> {
    let module = Module::from_binary(engine, module_bytes)?;
    let compiled = module.serialize()?;
    Ok(compiled)
}

pub async fn load_precompiled(
    engine: &Engine,
    compiled_bytes: &[u8],
) -> Result<Module> {
    // Much faster than compiling from scratch
    unsafe { Module::deserialize(engine, compiled_bytes) }
}
```

### 3. Memory Pooling

**Reuse WASM instances**:
```rust
pub struct WasmInstancePool {
    pool: Arc<Mutex<Vec<Instance>>>,
    max_size: usize,
}

impl WasmInstancePool {
    pub async fn acquire(&self, module: &Module, store: &mut Store) -> Result<Instance> {
        // Try to get from pool
        {
            let mut pool = self.pool.lock().await;
            if let Some(instance) = pool.pop() {
                // Reset instance state
                self.reset_instance(&instance, store)?;
                return Ok(instance);
            }
        }
        
        // Create new instance
        let instance = Instance::new(store, module, &[])?;
        Ok(instance)
    }
    
    pub async fn release(&self, instance: Instance) {
        let mut pool = self.pool.lock().await;
        if pool.len() < self.max_size {
            pool.push(instance);
        }
    }
}
```

### 4. Benchmarks

**Expected Performance**:
- Module load from cache: < 1ms
- Module compilation: 10-50ms (depending on size)
- Function call overhead: < 100μs
- Memory allocation: < 10μs
- Host function call: < 50μs

**Optimization Tips**:
- Keep modules small (< 1MB)
- Minimize host function calls
- Use bulk operations instead of loops
- Pre-allocate memory when possible

## Development Workflow

### 1. WASM Module Development

**Rust Example**:
```bash
# Create new WASM project
cargo new --lib my-wasm-module
cd my-wasm-module

# Add dependencies
cat >> Cargo.toml << EOF
[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
EOF

# Build for WASM
cargo build --target wasm32-wasi --release

# Optimize with wasm-opt
wasm-opt -Oz -o optimized.wasm target/wasm32-wasi/release/my_wasm_module.wasm
```

**JavaScript/TypeScript (AssemblyScript)**:
```bash
# Install AssemblyScript
npm install --save-dev assemblyscript

# Initialize project
npx asinit .

# Write code in assembly/index.ts
# Build
npm run asbuild

# Output: build/optimized.wasm
```

**Go (TinyGo)**:
```bash
# Install TinyGo
# https://tinygo.org/getting-started/install/

# Build for WASM
tinygo build -o module.wasm -target=wasi main.go
```

### 2. Testing WASM Modules

**Unit Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::*;
    
    #[tokio::test]
    async fn test_wasm_module() {
        let engine = Engine::default();
        let module = Module::from_file(&engine, "test.wasm").unwrap();
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).unwrap();
        
        let func = instance.get_typed_func::<(i32, i32), i32>(&mut store, "add").unwrap();
        let result = func.call(&mut store, (5, 3)).unwrap();
        
        assert_eq!(result, 8);
    }
}
```

**Integration Tests**:
```rust
#[tokio::test]
async fn test_wasm_executor_integration() {
    let minio = setup_test_minio().await;
    let executor = WasmExecutor::new(Arc::new(minio)).await.unwrap();
    
    let step = WasmJobStep {
        module_id: "test-module".to_string(),
        function_name: "process".to_string(),
        fuel_limit: Some(1_000_000),
        timeout_seconds: Some(10),
        memory_limit_mb: Some(64),
    };
    
    let mut context = JobContext::new();
    context.variables.insert("input".to_string(), json!({"value": 42}));
    
    let result = executor.execute(&step, &mut context).await.unwrap();
    
    assert_eq!(result.status, "success");
    assert_eq!(result.output["result"], 84);
}
```

### 3. Deployment

**Upload Module via API**:
```bash
# Upload WASM module
curl -X POST https://cron.example.com/api/v1/wasm/modules \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@optimized.wasm" \
  -F "name=customer-transformer" \
  -F "version=1.0.0" \
  -F "description=Transform customer data"

# Set permissions
curl -X POST https://cron.example.com/api/v1/wasm/modules/$MODULE_ID/permissions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "permissions": [
      {
        "type": "http:read",
        "resource_pattern": "https://api.example.com/*"
      },
      {
        "type": "database:write",
        "resource_pattern": "customer_analysis"
      }
    ]
  }'
```



## Technology Stack for WASM Integration

### Core Dependencies

```toml
[dependencies]
# WASM Runtime
wasmtime = "18.0"  # Latest stable Wasmtime runtime
wasmtime-wasi = "18.0"  # WASI support

# Async WASM support
async-trait = "0.1"

# Caching
lru = "0.12"  # LRU cache for compiled modules

# Cryptography for module verification
sha2 = "0.10"
ed25519-dalek = "2.1"  # For signature verification

# Existing dependencies
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
```

### WASM Toolchains

**Rust → WASM**:
```bash
rustup target add wasm32-wasi
cargo install wasm-opt
```

**JavaScript/TypeScript → WASM**:
```bash
npm install -g assemblyscript
```

**Go → WASM**:
```bash
# Install TinyGo
brew install tinygo  # macOS
```

**Python → WASM** (Experimental):
```bash
# Using Pyodide or RustPython
pip install pyodide-build
```

## Implementation Roadmap

### Phase 1: Core WASM Runtime (2-3 weeks)

**Tasks**:
1. ✅ Integrate Wasmtime runtime
2. ✅ Implement WasmExecutor with basic host functions
3. ✅ Add WASM job type to data models
4. ✅ Create database schema for WASM modules
5. ✅ Implement module caching
6. ✅ Add security limits (fuel, memory, timeout)

**Deliverables**:
- Working WASM executor
- Basic host functions (log, get_context, set_context)
- Module storage in MinIO
- Unit tests

### Phase 2: Security & Permissions (1-2 weeks)

**Tasks**:
1. ✅ Implement permission system
2. ✅ Add code signing and verification
3. ✅ Implement resource limits enforcement
4. ✅ Add audit logging
5. ✅ Security testing

**Deliverables**:
- Permission enforcement
- Module signature verification
- Comprehensive audit logs
- Security documentation

### Phase 3: Advanced Host Functions (2 weeks)

**Tasks**:
1. ✅ Implement http_request host function
2. ✅ Implement db_query host function
3. ✅ Implement file operations (read/write MinIO)
4. ✅ Add crypto utilities (hash, encrypt, decrypt)
5. ✅ Add time/date utilities

**Deliverables**:
- Full set of host functions
- Integration with existing executors
- Documentation and examples

### Phase 4: Developer Experience (2 weeks)

**Tasks**:
1. ✅ Create WASM module templates (Rust, JS, Go)
2. ✅ Build CLI tool for module development
3. ✅ Add module upload API
4. ✅ Create dashboard UI for module management
5. ✅ Write developer documentation

**Deliverables**:
- Module templates
- CLI tool
- API endpoints
- Dashboard integration
- Developer guide

### Phase 5: Performance Optimization (1-2 weeks)

**Tasks**:
1. ✅ Implement AOT compilation
2. ✅ Add instance pooling
3. ✅ Optimize memory allocation
4. ✅ Benchmark and profile
5. ✅ Performance tuning

**Deliverables**:
- Optimized runtime
- Performance benchmarks
- Optimization guide

### Phase 6: Production Readiness (1 week)

**Tasks**:
1. ✅ Integration tests
2. ✅ Load testing
3. ✅ Security audit
4. ✅ Documentation review
5. ✅ Production deployment guide

**Deliverables**:
- Test coverage > 80%
- Load test results
- Security audit report
- Complete documentation

## API Design

### WASM Module Management API

**Upload Module**:
```http
POST /api/v1/wasm/modules
Authorization: Bearer <token>
Content-Type: multipart/form-data

file: <wasm-file>
name: string
version: string
description: string (optional)
author: string
signature: string (hex-encoded)
```

**Response**:
```json
{
  "id": "uuid",
  "name": "customer-transformer",
  "version": "1.0.0",
  "minio_path": "wasm-modules/uuid.wasm",
  "hash": "sha256-hash",
  "size_bytes": 12345,
  "created_at": "2025-01-15T10:00:00Z"
}
```

**List Modules**:
```http
GET /api/v1/wasm/modules?page=1&limit=20
Authorization: Bearer <token>
```

**Get Module Details**:
```http
GET /api/v1/wasm/modules/{id}
Authorization: Bearer <token>
```

**Update Module Permissions**:
```http
PUT /api/v1/wasm/modules/{id}/permissions
Authorization: Bearer <token>
Content-Type: application/json

{
  "permissions": [
    {
      "type": "http:read",
      "resource_pattern": "https://api.example.com/*"
    },
    {
      "type": "database:write",
      "resource_pattern": "customer_*"
    }
  ]
}
```

**Delete Module**:
```http
DELETE /api/v1/wasm/modules/{id}
Authorization: Bearer <token>
```

**Download Module**:
```http
GET /api/v1/wasm/modules/{id}/download
Authorization: Bearer <token>
```

### WASM Execution Logs API

**Get Execution Logs**:
```http
GET /api/v1/wasm/executions/{execution_id}/logs
Authorization: Bearer <token>
```

**Response**:
```json
{
  "logs": [
    {
      "id": "uuid",
      "module_id": "uuid",
      "module_name": "customer-transformer",
      "fuel_consumed": 50000,
      "memory_used_bytes": 1048576,
      "duration_ms": 150,
      "error": null,
      "created_at": "2025-01-15T10:00:00Z"
    }
  ]
}
```

## Dashboard UI Design

### WASM Modules Page

**Features**:
- List all WASM modules with search and filter
- Upload new module with drag-and-drop
- View module details (version, size, permissions)
- Edit permissions
- Download module
- Delete module
- View execution statistics

**Mockup**:
```
┌─────────────────────────────────────────────────────────────┐
│ WASM Modules                                    [+ Upload]  │
├─────────────────────────────────────────────────────────────┤
│ Search: [________________]  Filter: [All ▼]                 │
├─────────────────────────────────────────────────────────────┤
│ Name                  Version  Size    Executions  Actions  │
├─────────────────────────────────────────────────────────────┤
│ customer-transformer  1.0.0    45 KB   1,234      [⚙️][📥][🗑️]│
│ order-validator       2.1.0    32 KB   567        [⚙️][📥][🗑️]│
│ data-enricher         1.5.2    78 KB   890        [⚙️][📥][🗑️]│
└─────────────────────────────────────────────────────────────┘
```

### Module Details Page

**Sections**:
1. **Overview**: Name, version, author, description, hash
2. **Permissions**: List of granted permissions
3. **Statistics**: Total executions, avg duration, fuel consumption
4. **Recent Executions**: Last 10 executions with status
5. **Code**: View WASM binary info (imports, exports, functions)

### Job Builder Integration

**WASM Step Configuration**:
```
┌─────────────────────────────────────────────────────────────┐
│ Add Step: WASM Module                                       │
├─────────────────────────────────────────────────────────────┤
│ Step Name: [Transform Customer Data___________________]     │
│                                                              │
│ Module: [customer-transformer v1.0.0 ▼]                    │
│                                                              │
│ Function: [transform_data_________________________]         │
│                                                              │
│ Fuel Limit: [1000000_____] instructions                    │
│                                                              │
│ Timeout: [30____] seconds                                   │
│                                                              │
│ Memory Limit: [64____] MB                                   │
│                                                              │
│ [Cancel]                                    [Add Step]      │
└─────────────────────────────────────────────────────────────┘
```

## CLI Tool Design

### Installation

```bash
# Install via cargo
cargo install vietnam-cron-wasm-cli

# Or download binary
curl -L https://github.com/example/vietnam-cron/releases/download/v1.0.0/wasm-cli-macos -o wasm-cli
chmod +x wasm-cli
```

### Commands

**Initialize Project**:
```bash
wasm-cli init my-module --lang rust
# Creates project structure with template
```

**Build Module**:
```bash
wasm-cli build
# Compiles and optimizes WASM module
```

**Test Module Locally**:
```bash
wasm-cli test --input test-context.json
# Runs module with test input
```

**Upload Module**:
```bash
wasm-cli upload \
  --url https://cron.example.com \
  --token $TOKEN \
  --file optimized.wasm \
  --name customer-transformer \
  --version 1.0.0
```

**List Modules**:
```bash
wasm-cli list --url https://cron.example.com --token $TOKEN
```

**Download Module**:
```bash
wasm-cli download --id <module-id> --output module.wasm
```

## Benefits Summary

### 1. Flexibility
- ✅ Support multiple programming languages (Rust, Go, JS, Python)
- ✅ Custom business logic without rebuilding system
- ✅ Rapid iteration and deployment

### 2. Security
- ✅ Sandboxed execution environment
- ✅ Fine-grained permission system
- ✅ Resource limits prevent abuse
- ✅ Code signing and verification

### 3. Performance
- ✅ Near-native execution speed
- ✅ Module caching and AOT compilation
- ✅ Minimal overhead (< 100μs per call)
- ✅ Efficient memory usage

### 4. Developer Experience
- ✅ Familiar programming languages
- ✅ Rich tooling and templates
- ✅ Easy testing and debugging
- ✅ Clear documentation

### 5. Extensibility
- ✅ Plugin architecture
- ✅ Third-party module marketplace
- ✅ Community contributions
- ✅ Future-proof design

## Risks and Mitigations

### Risk 1: Security Vulnerabilities

**Mitigation**:
- Regular security audits
- Automated vulnerability scanning
- Strict permission enforcement
- Code review for all modules
- Bug bounty program

### Risk 2: Performance Overhead

**Mitigation**:
- Comprehensive benchmarking
- Module caching and AOT compilation
- Performance monitoring and alerting
- Optimization guidelines for developers

### Risk 3: Complexity

**Mitigation**:
- Clear documentation and examples
- Developer training and support
- Gradual rollout (opt-in feature)
- Fallback to existing executors

### Risk 4: Module Quality

**Mitigation**:
- Module review process
- Automated testing requirements
- Community ratings and reviews
- Official certified modules

## Alternatives Considered

### 1. Lua Scripting
**Pros**: Lightweight, easy to embed
**Cons**: Single language, less secure, slower than WASM

### 2. JavaScript (V8/Deno)
**Pros**: Popular language, good tooling
**Cons**: Heavier runtime, JavaScript-only, more attack surface

### 3. Python (PyO3)
**Pros**: Popular for data processing
**Cons**: GIL issues, slower, larger memory footprint

### 4. Native Plugins (Dynamic Libraries)
**Pros**: Maximum performance
**Cons**: No sandboxing, platform-specific, security risks

**Conclusion**: WASM provides the best balance of security, performance, and flexibility.

## Conclusion

WebAssembly integration sẽ biến Vietnam Enterprise Cron System thành một platform mở rộng được, cho phép users tự định nghĩa business logic phức tạp một cách an toàn và hiệu quả. Với kiến trúc sandbox, permission system, và support đa ngôn ngữ, WASM là lựa chọn tối ưu cho enterprise use cases.

**Recommended Next Steps**:
1. Review và approve design document này
2. Tạo POC (Proof of Concept) với Phase 1 tasks
3. Validate performance benchmarks
4. Security review với team
5. Plan rollout strategy

---

**Document Version**: 1.0  
**Last Updated**: 2025-01-15  
**Author**: Kiro AI Assistant  
**Status**: Draft - Awaiting Review
