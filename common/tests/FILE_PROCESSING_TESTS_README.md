# File Processing Property Tests

## Overview

This document describes the property-based tests for file processing operations (Excel and CSV) in the Vietnam Enterprise Cron System.

## Test File

`common/tests/file_processing_property_tests.rs`

## Properties Tested

### Property 96: Excel file reading
**Validates: Requirements 15.1**

*For any* valid XLSX file in MinIO, the Worker should successfully read and parse it.

- Tests that Excel files can be written and then read back
- Verifies the read operation completes without errors
- Uses proptest with 100 iterations

### Property 97: Excel data structure preservation
**Validates: Requirements 15.2**

*For any* Excel file parsed to JSON, the structure (sheets, rows, columns) should be preserved in the Job Context.

- Tests that row count is preserved after write/read cycle
- Tests that column count is preserved
- Verifies data structure integrity

### Property 98: CSV file reading
**Validates: Requirements 15.3**

*For any* valid CSV file in MinIO, the Worker should successfully read and parse it.

- Tests CSV files with different delimiters (comma, semicolon, tab)
- Verifies read operation completes without errors
- Uses proptest with 100 iterations

### Property 99: CSV delimiter support
**Validates: Requirements 15.4**

*For any* CSV file with delimiter D (comma, semicolon, tab), parsing with delimiter D should correctly parse all rows.

- Tests all three supported delimiters
- Verifies row count preservation with each delimiter
- Ensures delimiter-specific parsing works correctly

### Property 100: Excel sheet selection
**Validates: Requirements 15.5**

*For any* Excel file and sheet selector (name or index), only that sheet's data should be present in the output.

- Tests sheet selection by index
- Verifies only one sheet is returned when specified
- Tests with sheet_index parameter

### Property 101: Data transformation application
**Validates: Requirements 15.6**

*For any* transformation rule applied to file data, the output in Job Context should reflect the transformation.

- Tests column mapping transformations
- Verifies transformations are applied without errors
- Tests transformation pipeline

### Property 102: Excel write round-trip
**Validates: Requirements 15.7**

*For any* data written to Excel format then read back, the data should be preserved (round-trip consistency).

- Tests write then read cycle
- Verifies row count matches original
- Ensures data integrity through round-trip

### Property 103: CSV write round-trip
**Validates: Requirements 15.8**

*For any* data written to CSV format then read back, the data should be preserved (round-trip consistency).

- Tests write then read cycle with all delimiters
- Verifies row count matches original
- Ensures data integrity through round-trip

### Property 104: File output path format
**Validates: Requirements 15.9**

*For any* file written to MinIO, the path should follow the format `jobs/{job_id}/executions/{execution_id}/output/{filename}`.

- Tests path format compliance
- Verifies correct path structure
- Ensures consistent path generation

### Property 105: File metadata storage
**Validates: Requirements 15.10**

*For any* file processing step completion, the MinIO file path and row count should be present in the Job Context.

- Tests metadata presence in output
- Verifies file_size, row_count, and path are stored
- Checks Job Context file metadata array

### Property 106: Invalid file format error handling
**Validates: Requirements 15.11**

*For any* invalid file format encountered, the Worker should fail with a clear error message indicating the parsing error.

- Tests with random invalid data
- Verifies error is returned (not panic)
- Checks error message clarity

## Edge Case Tests

### test_empty_excel_data
Tests that empty Excel data can be written and read without errors.

### test_missing_source_path_error
Tests that Read operation without source_path returns appropriate error.

### test_missing_destination_path_error
Tests that Write operation without destination_path returns appropriate error.

### test_missing_write_data_error
Tests that Write operation without write_data variable returns appropriate error.

## Running the Tests

### Compile Check
```bash
cargo test --test file_processing_property_tests --no-run
```

### Run Tests (Requires MinIO)
```bash
# Start MinIO testcontainer first
docker run -d -p 9000:9000 -p 9001:9001 \
  -e MINIO_ROOT_USER=minioadmin \
  -e MINIO_ROOT_PASSWORD=minioadmin \
  minio/minio server /data --console-address ":9001"

# Create test bucket
mc alias set local http://localhost:9000 minioadmin minioadmin
mc mb local/test-bucket

# Run tests
cargo test --test file_processing_property_tests -- --ignored --nocapture
```

## Test Infrastructure Requirements

- **MinIO**: S3-compatible object storage for file operations
- **Proptest**: Property-based testing framework (100 iterations per property)
- **Tokio Runtime**: Async runtime for test execution

## Test Status

All tests are marked with `#[ignore]` because they require MinIO testcontainer infrastructure. They compile successfully and are ready to run once the infrastructure is available.

## Notes

- Tests use proptest with 100 iterations as specified in the design document
- Each property test is tagged with the feature name and property number
- Tests validate both success cases and error handling
- Round-trip tests ensure data integrity through write/read cycles
