# Implementation Plan

- [ ] 1. Set up WASM runtime infrastructure and core data models
  - Add wasmtime and wasmtime-wasi dependencies to Cargo.toml
  - Create `common/src/executor/wasm.rs` module
  - Define WasmJobStep, WasmModule, WasmPermission, WasmExecutionLog data models
  - Extend JobType enum to include Wasm variant
  - Add WasmError types to common/src/errors.rs
  - _Requirements: 1.5, 6.4_

- [ ] 2. Create database schema for WASM modules
  - Create migration for wasm_modules table
  - Create migration for wasm_module_permissions table
  - Create migration for wasm_execution_logs table
  - Add indexes for performance
  - _Requirements: 6.4, 4.7, 8.1_

- [ ] 3. Implement WasmModuleRepository for database operations
  - Create `common/src/db/repositories/wasm_module.rs`
  - Implement save_module, find_by_id, find_all, delete methods
  - Implement save_permissions, find_permissions methods
  - Implement log_execution, find_execution_logs methods
  - Use compile-time query checking with sqlx
  - _Requirements: 6.4, 6.6, 6.7, 6.8, 8.1, 8.6_

- [ ] 3.1 Write property test for WasmModuleRepository
  - **Property 22: Module Metadata Persistence**
  - **Property 24: Module Listing**
  - **Property 25: Module Deletion**
  - **Property 26: Permission Update Persistence**
  - **Validates: Requirements 6.4, 6.6, 6.7, 6.8**

- [ ] 4. Implement ModuleCache with LRU eviction
  - Create module cache structure using lru crate
  - Implement get, put, clear methods
  - Add thread-safe access with RwLock
  - Configure cache size from settings
  - _Requirements: 7.1, 7.2_

- [ ] 4.1 Write property test for ModuleCache
  - **Property 27: Module Caching**
  - **Property 28: LRU Cache Eviction**
  - **Validates: Requirements 7.1, 7.2**

- [ ] 5. Implement WasmState and security configuration
  - Create WasmState struct with JobContext and WasiCtx
  - Create WasmSecurityConfig with default limits
  - Add configuration section to config.toml
  - Load WASM config in Settings struct
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 6. Implement core WasmExecutor structure
  - Create WasmExecutor struct with Engine, ModuleCache, MinIO client, DB pool
  - Implement new() method to initialize Wasmtime engine with security config
  - Configure fuel metering, memory limits, and async support
  - Set up WASI context builder
  - _Requirements: 1.1, 1.2, 3.1, 3.2, 3.3_

- [ ] 7. Implement module loading and compilation
  - Implement load_module() to fetch from MinIO or cache
  - Add module compilation with Wasmtime
  - Implement cache storage after compilation
  - Add error handling for invalid modules
  - _Requirements: 1.1, 1.2, 7.1_

- [ ] 7.1 Write property test for module loading
  - **Property 1: Module Loading from MinIO**
  - **Property 2: Module Compilation**
  - **Validates: Requirements 1.1, 1.2**

- [ ] 8. Implement permission checking system
  - Implement check_permission() method
  - Load module permissions from database
  - Match resource patterns using glob matching
  - Return PermissionDenied errors for unauthorized access
  - _Requirements: 4.7, 4.8_

- [ ] 8.1 Write property test for permission enforcement
  - **Property 12: Permission Verification**
  - **Property 13: Unauthorized Operation Denial**
  - **Validates: Requirements 4.7, 4.8**

- [ ] 9. Implement host function: log
  - Create host_log function to write to tracing system
  - Extract message from WASM memory
  - Map log levels (0=debug, 1=info, 2=warn, 3=error)
  - Add structured logging with module context
  - _Requirements: 5.1_

- [ ] 9.1 Write property test for log host function
  - **Property 14: Log Host Function**
  - **Validates: Requirements 5.1**

- [ ] 10. Implement host functions: get_context and set_context
  - Create host_get_context to retrieve values from JobContext
  - Create host_set_context to update JobContext
  - Handle memory allocation for return values
  - Add error handling for missing keys
  - _Requirements: 5.2, 5.3_

- [ ] 10.1 Write property test for context host functions
  - **Property 15: Get Context Host Function**
  - **Property 16: Set Context Host Function**
  - **Validates: Requirements 5.2, 5.3**

- [ ] 11. Implement host function: http_request
  - Create host_http_request to execute HTTP requests
  - Parse HTTP config from WASM memory
  - Check http:read or http:write permissions
  - Execute request using reqwest client
  - Return response as JSON in WASM memory
  - _Requirements: 5.4_

- [ ] 11.1 Write property test for http_request host function
  - **Property 17: HTTP Request Host Function**
  - **Validates: Requirements 5.4**

- [ ] 12. Implement host function: db_query
  - Create host_db_query to execute database queries
  - Parse database config from WASM memory
  - Check database:read or database:write permissions
  - Execute query using appropriate database driver
  - Return result as JSON in WASM memory
  - _Requirements: 5.5_

- [ ] 12.1 Write property test for db_query host function
  - **Property 18: Database Query Host Function**
  - **Validates: Requirements 5.5**

- [ ] 13. Implement WASM module instantiation with host functions
  - Implement instantiate_with_host_functions() method
  - Link all host functions to WASM instance
  - Set up WASI context with job environment variables
  - Configure store with fuel limit and timeout
  - _Requirements: 1.2, 2.5, 3.1, 3.3_

- [ ] 14. Implement main execute() method for WasmExecutor
  - Implement execute() method for WasmJobStep
  - Serialize JobContext to JSON input
  - Call WASM function with input
  - Parse JSON output and update JobContext
  - Handle timeouts with tokio::time::timeout
  - _Requirements: 1.3, 1.4, 3.3_

- [ ] 14.1 Write property test for context serialization
  - **Property 3: Context Serialization to Module**
  - **Property 4: Output Parsing and Context Update**
  - **Validates: Requirements 1.3, 1.4**

- [ ] 15. Implement resource limit enforcement
  - Add fuel consumption tracking
  - Enforce memory limits during execution
  - Implement timeout handling
  - Return specific errors for each limit type
  - Terminate execution on limit exceeded
  - _Requirements: 3.1, 3.2, 3.3, 3.5_

- [ ] 15.1 Write property test for resource limits
  - **Property 7: Fuel Limit Enforcement**
  - **Property 8: Memory Limit Enforcement**
  - **Property 9: Timeout Enforcement**
  - **Property 11: Resource Limit Termination**
  - **Validates: Requirements 3.1, 3.2, 3.3, 3.5**

- [ ] 16. Implement execution logging
  - Log execution start with execution_id and module_id
  - Track fuel consumed, memory used, duration
  - Log errors on failure
  - Persist logs to wasm_execution_logs table
  - Add tracing instrumentation
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [ ] 16.1 Write property test for execution logging
  - **Property 31: Execution Logging**
  - **Property 32: Fuel Consumption Logging**
  - **Property 33: Memory Usage Logging**
  - **Property 34: Duration Logging**
  - **Property 35: Error Logging**
  - **Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.5**

- [ ] 17. Implement JobExecutor trait for WasmExecutor
  - Implement async execute() method from JobExecutor trait
  - Match on JobType::Wasm variant
  - Integrate with existing job execution engine
  - Return StepOutput with status and output
  - _Requirements: 1.1, 13.2_

- [ ] 18. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 19. Implement module upload validation
  - Create validate_wasm_module() function
  - Use Wasmtime to validate module format
  - Check module size limits
  - Perform static analysis for dangerous patterns
  - Return validation errors
  - _Requirements: 6.1_

- [ ] 19.1 Write property test for module validation
  - **Property 19: Module Format Validation**
  - **Validates: Requirements 6.1**

- [ ] 20. Implement module hash calculation and verification
  - Create calculate_hash() function using SHA256
  - Store hash in database on upload
  - Implement verify_hash() function
  - Check hash on module load
  - Log security alerts on mismatch
  - _Requirements: 6.2, 14.3, 14.4, 14.5_

- [ ] 20.1 Write property test for hash operations
  - **Property 20: Module Hash Calculation**
  - **Property 52: Hash Verification on Load**
  - **Property 53: Hash Verification Failure Response**
  - **Validates: Requirements 6.2, 14.3, 14.4, 14.5**

- [ ] 21. Implement cryptographic signature verification
  - Create verify_signature() function using ed25519-dalek
  - Parse signature and public key
  - Verify signature on module upload
  - Reject modules with invalid signatures
  - Make signature verification optional via config
  - _Requirements: 6.5, 14.1, 14.2_

- [ ] 21.1 Write property test for signature verification
  - **Property 23: Signature Verification**
  - **Validates: Requirements 6.5, 14.1, 14.2**

- [ ] 22. Implement API handler: upload_module
  - Create POST /api/v1/wasm/modules endpoint
  - Parse multipart form data
  - Validate module format
  - Calculate hash
  - Verify signature if provided
  - Store module in MinIO
  - Save metadata to database
  - Return module details
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [ ] 22.1 Write property test for module upload
  - **Property 21: Module Storage in MinIO**
  - **Validates: Requirements 6.3**

- [ ] 23. Implement API handler: list_modules
  - Create GET /api/v1/wasm/modules endpoint
  - Support pagination with page and limit params
  - Query modules from database
  - Return module list with metadata
  - _Requirements: 6.6_

- [ ] 24. Implement API handler: get_module
  - Create GET /api/v1/wasm/modules/{id} endpoint
  - Query module by ID from database
  - Include permissions in response
  - Return 404 if not found
  - _Requirements: 6.6_

- [ ] 25. Implement API handler: update_permissions
  - Create PUT /api/v1/wasm/modules/{id}/permissions endpoint
  - Parse permission list from request body
  - Validate permission types and patterns
  - Delete existing permissions
  - Insert new permissions
  - Return success status
  - _Requirements: 6.8_

- [ ] 26. Implement API handler: delete_module
  - Create DELETE /api/v1/wasm/modules/{id} endpoint
  - Delete module from database (cascades to permissions and logs)
  - Delete module binary from MinIO
  - Return success status
  - _Requirements: 6.7_

- [ ] 27. Implement API handler: download_module
  - Create GET /api/v1/wasm/modules/{id}/download endpoint
  - Query module metadata from database
  - Load module binary from MinIO
  - Return binary with appropriate content-type
  - _Requirements: 6.6_

- [ ] 28. Implement API handler: get_execution_logs
  - Create GET /api/v1/wasm/executions/{execution_id}/logs endpoint
  - Query logs from database filtered by execution_id
  - Support filtering by module_id via query param
  - Return log entries with metrics
  - _Requirements: 8.6_

- [ ] 28.1 Write property test for log filtering
  - **Property 36: Execution Log Filtering**
  - **Validates: Requirements 8.6**

- [ ] 29. Add WASM routes to API router
  - Register all WASM endpoints in api/src/routes.rs
  - Apply authentication middleware
  - Apply RBAC middleware (admin only for upload/delete)
  - Add rate limiting
  - _Requirements: 6.1, 6.6, 6.7, 6.8_

- [ ] 30. Implement multi-step job integration
  - Ensure WASM steps receive output from previous steps in context
  - Ensure WASM step output is available to subsequent steps
  - Validate job definitions with WASM steps
  - Support variable resolution in WASM step config
  - _Requirements: 11.1, 11.2, 13.1, 13.3_

- [ ] 30.1 Write property test for multi-step integration
  - **Property 37: Context Propagation in Multi-Step Jobs**
  - **Property 38: Transformed Data Availability**
  - **Property 41: Step Execution Order**
  - **Property 47: Job Definition Validation**
  - **Property 48: WASM Step Integration**
  - **Property 49: Variable Resolution**
  - **Validates: Requirements 11.1, 11.2, 11.5, 13.1, 13.2, 13.3**

- [ ] 31. Implement data transformation support
  - Allow WASM modules to transform step output
  - Merge transformed data into job context
  - Preserve original data unless explicitly overwritten
  - _Requirements: 11.2, 11.4_

- [ ] 31.1 Write property test for data transformation
  - **Property 40: Data Enrichment Merging**
  - **Validates: Requirements 11.4**

- [ ] 32. Implement validation error handling
  - Allow WASM modules to return validation errors
  - Parse validation errors from module output
  - Fail job with validation errors
  - Include validation errors in execution result
  - _Requirements: 11.3_

- [ ] 32.1 Write property test for validation errors
  - **Property 39: Validation Error Handling**
  - **Validates: Requirements 11.3**

- [ ] 33. Implement conditional routing support
  - Parse routing decisions from WASM module output
  - Support single next step routing
  - Support multiple next steps routing
  - Support terminal routing (no next steps)
  - Validate routing decisions
  - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_

- [ ] 33.1 Write property test for routing logic
  - **Property 42: Routing Decision Parsing**
  - **Property 43: Single Next Step Routing**
  - **Property 44: Multiple Next Steps Routing**
  - **Property 45: Terminal Routing Completion**
  - **Property 46: Invalid Routing Error Handling**
  - **Validates: Requirements 12.1, 12.2, 12.3, 12.4, 12.5**

- [ ] 34. Implement retry and error handling for WASM steps
  - Apply exponential backoff retry logic
  - Use same retry configuration as other step types
  - Integrate with circuit breaker
  - Update job execution status on failure
  - Persist step output on completion
  - _Requirements: 13.4, 13.5_

- [ ] 34.1 Write property test for error handling consistency
  - **Property 50: Status Update and Output Persistence**
  - **Property 51: Retry and Error Handling Consistency**
  - **Validates: Requirements 13.4, 13.5**

- [ ] 35. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 36. Create WASM module templates
  - Create Rust template with Cargo.toml and example code
  - Create Go/TinyGo template with go.mod and example code
  - Create JavaScript/AssemblyScript template with package.json and example code
  - Include examples of calling host functions
  - Add build scripts for each language
  - _Requirements: 15.1, 15.2, 15.3, 15.4_

- [ ] 37. Create CLI tool structure
  - Create new binary crate for CLI tool
  - Add clap for command-line parsing
  - Implement init command to generate project from template
  - Implement build command to compile and optimize WASM
  - Implement test command to run module locally
  - Implement upload command to upload to server
  - Implement list command to list modules
  - Implement download command to download module
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 15.1_

- [ ] 38. Implement dashboard UI for WASM modules
  - Create wasm_modules.html template
  - Add module list view with search and filter
  - Add module upload form with drag-and-drop
  - Create module_details.html template
  - Add permission editing form
  - Add module deletion with confirmation
  - Add execution statistics display
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 39. Add WASM step configuration to job builder UI
  - Add WASM step type to job builder
  - Create form for WASM step configuration
  - Add module selector dropdown
  - Add fields for function name, fuel limit, timeout, memory limit
  - Validate WASM step configuration
  - _Requirements: 1.5_

- [ ] 40. Add Prometheus metrics for WASM execution
  - Add counter for total WASM executions
  - Add histogram for execution duration
  - Add gauge for cache size and hit rate
  - Add counter for fuel consumed
  - Add counter for permission denials
  - Add counter for security alerts
  - _Requirements: 8.1_

- [ ] 41. Add OpenTelemetry tracing for WASM execution
  - Add span for module loading
  - Add span for module compilation
  - Add span for module execution
  - Add span for host function calls
  - Include module_id, execution_id in span attributes
  - _Requirements: 8.1_

- [ ] 42. Write integration tests for end-to-end WASM execution
  - Test complete job execution with WASM step
  - Test multi-step job with WASM and other step types
  - Test module upload, execution, and deletion workflow
  - Test permission enforcement across full stack
  - Test error handling and retry logic

- [ ] 43. Write performance tests
  - **Property 29: Cache Retrieval Performance**
  - **Property 30: Compilation Performance**
  - Benchmark module loading and compilation
  - Benchmark execution overhead
  - Benchmark host function calls
  - Verify performance meets requirements
  - **Validates: Requirements 7.3, 7.4**

- [ ] 44. Create developer documentation
  - Write getting started guide
  - Document host function API
  - Provide examples for common use cases (transformation, validation, routing)
  - Document permission system
  - Document resource limits and best practices
  - Document CLI tool usage
  - _Requirements: 15.5_

- [ ] 45. Create operator documentation
  - Document WASM configuration options
  - Document deployment considerations
  - Document monitoring and alerting
  - Document security best practices
  - Document troubleshooting guide
  - _Requirements: 6.1, 6.6, 6.7, 6.8_

- [ ] 46. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.
