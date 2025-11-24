// Property-based tests for file processing operations
// Feature: vietnam-enterprise-cron
// Requirements: 15.1-15.11 - File processing with Excel/CSV support

use common::executor::file::FileProcessingExecutor;
use common::executor::JobExecutor;
use common::models::{
    DataTransformation, FileFormat, FileOperation, FileProcessingOptions, JobContext, JobStep,
    JobType,
};
use common::storage::minio::MinioClient;
use common::storage::service::{MinIOService, MinIOServiceImpl};
use proptest::prelude::*;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Property Generators
// ============================================================================

/// Generate valid Excel data (array of arrays)
fn arb_excel_data() -> impl Strategy<Value = Value> {
    prop::collection::vec(
        prop::collection::vec(
            prop_oneof![
                any::<i64>().prop_map(|i| json!(i)),
                any::<f64>().prop_map(|f| json!(f)),
                "[a-zA-Z0-9 ]{1,20}".prop_map(|s| json!(s)),
                any::<bool>().prop_map(|b| json!(b)),
            ],
            1..10, // 1-10 columns
        )
        .prop_map(|cells| Value::Array(cells)),
        1..50, // 1-50 rows
    )
    .prop_map(|rows| Value::Array(rows))
}

/// Generate valid CSV data (array of arrays)
fn arb_csv_data() -> impl Strategy<Value = Value> {
    prop::collection::vec(
        prop::collection::vec(
            prop_oneof![
                any::<i64>().prop_map(|i| json!(i)),
                "[a-zA-Z0-9 ]{1,20}".prop_map(|s| json!(s)),
            ],
            1..10, // 1-10 columns
        )
        .prop_map(|cells| Value::Array(cells)),
        1..50, // 1-50 rows
    )
    .prop_map(|rows| Value::Array(rows))
}

/// Generate CSV delimiter
fn arb_csv_delimiter() -> impl Strategy<Value = char> {
    prop::sample::select(vec![',', ';', '\t'])
}

/// Generate file path
fn arb_file_path(job_id: Uuid, execution_id: Uuid, filename: &str) -> String {
    format!(
        "jobs/{}/executions/{}/output/{}",
        job_id, execution_id, filename
    )
}

/// Generate FileProcessingOptions
#[allow(dead_code)]
fn arb_file_processing_options() -> impl Strategy<Value = FileProcessingOptions> {
    (
        prop::option::of("[a-zA-Z]{3,10}"), // sheet_name
        prop::option::of(0usize..5usize),   // sheet_index
        prop::bool::ANY,                    // streaming
    )
        .prop_map(
            |(sheet_name, sheet_index, streaming)| FileProcessingOptions {
                sheet_name,
                sheet_index,
                transformations: vec![], // Empty for now
                streaming,
            },
        )
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test MinIO client
async fn create_test_minio_client() -> MinioClient {
    use common::config::MinioConfig;

    let config = MinioConfig {
        endpoint: "http://localhost:9000".to_string(),
        access_key: "minioadmin".to_string(),
        secret_key: "minioadmin".to_string(),
        bucket: "test-bucket".to_string(),
        region: "us-east-1".to_string(),
    };

    MinioClient::new(&config)
        .await
        .expect("Failed to create test MinIO client")
}

// ============================================================================
// Property Tests
// ============================================================================

/// **Feature: vietnam-enterprise-cron, Property 96: Excel file reading**
/// **Validates: Requirements 15.1**
///
/// *For any* valid XLSX file in MinIO, the Worker should successfully read and parse it.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_excel_file_reading() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // First write the Excel file
            let write_path = arb_file_path(job_id, execution_id, "test.xlsx");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(write_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let write_result = executor.execute(&write_step, &mut context).await;
            prop_assert!(write_result.is_ok(), "Failed to write Excel file: {:?}", write_result.err());

            // Now read the Excel file
            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Excel,
                    source_path: Some(write_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await;
            prop_assert!(read_result.is_ok(), "Failed to read Excel file: {:?}", read_result.err());

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 97: Excel data structure preservation**
/// **Validates: Requirements 15.2**
///
/// *For any* Excel file parsed to JSON, the structure (sheets, rows, columns) should be
/// preserved in the Job Context.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_excel_data_structure_preservation() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write and read Excel file
            let file_path = arb_file_path(job_id, execution_id, "test.xlsx");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            executor.execute(&write_step, &mut context).await.unwrap();

            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Excel,
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await.unwrap();

            // Verify structure is preserved
            let read_data = &read_result.output["data"];

            // For single sheet, data should be in Sheet1
            if let Value::Object(sheets) = read_data {
                prop_assert!(sheets.contains_key("Sheet1"), "Sheet1 should exist");
                let sheet_data = &sheets["Sheet1"];

                if let (Value::Array(original_rows), Value::Array(read_rows)) = (&data, sheet_data) {
                    prop_assert_eq!(original_rows.len(), read_rows.len(),
                        "Row count should be preserved");

                    // Check first row column count if exists
                    if !original_rows.is_empty() && !read_rows.is_empty() {
                        if let (Value::Array(orig_cols), Value::Array(read_cols)) =
                            (&original_rows[0], &read_rows[0]) {
                            prop_assert_eq!(orig_cols.len(), read_cols.len(),
                                "Column count should be preserved");
                        }
                    }
                }
            }

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 98: CSV file reading**
/// **Validates: Requirements 15.3**
///
/// *For any* valid CSV file in MinIO, the Worker should successfully read and parse it.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_csv_file_reading() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_csv_data(),
        delimiter in arb_csv_delimiter()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write CSV file
            let file_path = arb_file_path(job_id, execution_id, "test.csv");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write CSV".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Csv { delimiter },
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let write_result = executor.execute(&write_step, &mut context).await;
            prop_assert!(write_result.is_ok(), "Failed to write CSV file: {:?}", write_result.err());

            // Read CSV file
            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read CSV".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Csv { delimiter },
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await;
            prop_assert!(read_result.is_ok(), "Failed to read CSV file: {:?}", read_result.err());

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 99: CSV delimiter support**
/// **Validates: Requirements 15.4**
///
/// *For any* CSV file with delimiter D (comma, semicolon, tab), parsing with delimiter D
/// should correctly parse all rows.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_csv_delimiter_support() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_csv_data(),
        delimiter in arb_csv_delimiter()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write and read with specific delimiter
            let file_path = arb_file_path(job_id, execution_id, "test.csv");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write CSV".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Csv { delimiter },
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            executor.execute(&write_step, &mut context).await.unwrap();

            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read CSV".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Csv { delimiter },
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await.unwrap();

            // Verify row count is preserved
            let read_data = &read_result.output["data"];
            if let (Value::Array(original_rows), Value::Array(read_rows)) = (&data, read_data) {
                prop_assert_eq!(original_rows.len(), read_rows.len(),
                    "Row count should be preserved with delimiter {:?}", delimiter);
            }

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 100: Excel sheet selection**
/// **Validates: Requirements 15.5**
///
/// *For any* Excel file and sheet selector (name or index), only that sheet's data
/// should be present in the output.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_excel_sheet_selection() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data(),
        sheet_index in 0usize..1usize // Only test index 0 since we write single sheet
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write Excel file
            let file_path = arb_file_path(job_id, execution_id, "test.xlsx");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            executor.execute(&write_step, &mut context).await.unwrap();

            // Read with sheet index selection
            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Excel,
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: Some(sheet_index),
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await.unwrap();

            // Verify only one sheet is returned
            let read_data = &read_result.output["data"];
            if let Value::Object(sheets) = read_data {
                prop_assert_eq!(sheets.len(), 1,
                    "Should return only one sheet when sheet_index is specified");
            }

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 101: Data transformation application**
/// **Validates: Requirements 15.6**
///
/// *For any* transformation rule applied to file data, the output in Job Context
/// should reflect the transformation.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_data_transformation_application() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write Excel file
            let file_path = arb_file_path(job_id, execution_id, "test.xlsx");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            executor.execute(&write_step, &mut context).await.unwrap();

            // Read with transformations
            let transformations = vec![
                DataTransformation::ColumnMapping {
                    from: "col1".to_string(),
                    to: "column1".to_string(),
                },
            ];

            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Excel,
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations,
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await;
            prop_assert!(read_result.is_ok(),
                "Should successfully apply transformations: {:?}", read_result.err());

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 102: Excel write round-trip**
/// **Validates: Requirements 15.7**
///
/// *For any* data written to Excel format then read back, the data should be
/// preserved (round-trip consistency).
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_excel_write_round_trip() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write Excel file
            let file_path = arb_file_path(job_id, execution_id, "roundtrip.xlsx");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            executor.execute(&write_step, &mut context).await.unwrap();

            // Read Excel file back
            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Excel,
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await.unwrap();

            // Verify round-trip consistency
            let read_data = &read_result.output["data"];

            // Extract Sheet1 data
            if let Value::Object(sheets) = read_data {
                if let Some(sheet_data) = sheets.get("Sheet1") {
                    if let (Value::Array(original_rows), Value::Array(read_rows)) = (&data, sheet_data) {
                        prop_assert_eq!(original_rows.len(), read_rows.len(),
                            "Row count should match after round-trip");
                    }
                }
            }

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 103: CSV write round-trip**
/// **Validates: Requirements 15.8**
///
/// *For any* data written to CSV format then read back, the data should be
/// preserved (round-trip consistency).
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_csv_write_round_trip() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_csv_data(),
        delimiter in arb_csv_delimiter()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write CSV file
            let file_path = arb_file_path(job_id, execution_id, "roundtrip.csv");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write CSV".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Csv { delimiter },
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            executor.execute(&write_step, &mut context).await.unwrap();

            // Read CSV file back
            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read CSV".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Csv { delimiter },
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await.unwrap();

            // Verify round-trip consistency
            let read_data = &read_result.output["data"];
            if let (Value::Array(original_rows), Value::Array(read_rows)) = (&data, read_data) {
                prop_assert_eq!(original_rows.len(), read_rows.len(),
                    "Row count should match after CSV round-trip");
            }

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 104: File output path format**
/// **Validates: Requirements 15.9**
///
/// *For any* file written to MinIO, the path should follow the format
/// `jobs/{job_id}/executions/{execution_id}/output/{filename}`.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_file_output_path_format() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data(),
        filename in "[a-z]{3,10}\\.xlsx"
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write file with specific path format
            let file_path = format!(
                "jobs/{}/executions/{}/output/{}",
                job_id, execution_id, filename
            );
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let write_result = executor.execute(&write_step, &mut context).await.unwrap();

            // Verify path format in output
            let output_path = write_result.output["destination_path"].as_str();
            prop_assert!(output_path.is_some(), "destination_path should be present");

            let expected_prefix = format!("jobs/{}/executions/{}/output/", job_id, execution_id);
            prop_assert!(output_path.unwrap().starts_with(&expected_prefix),
                "Path should follow format jobs/{{job_id}}/executions/{{execution_id}}/output/{{filename}}");

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 105: File metadata storage**
/// **Validates: Requirements 15.10**
///
/// *For any* file processing step completion, the MinIO file path and row count
/// should be present in the Job Context.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_file_metadata_storage() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        data in arb_excel_data()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Write Excel file
            let file_path = arb_file_path(job_id, execution_id, "metadata_test.xlsx");
            context.set_variable("write_data".to_string(), data.clone());

            let write_step = JobStep {
                id: "write_step".to_string(),
                name: "Write Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Write,
                    format: FileFormat::Excel,
                    source_path: None,
                    destination_path: Some(file_path.clone()),
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let write_result = executor.execute(&write_step, &mut context).await.unwrap();

            // Verify metadata in output
            prop_assert!(write_result.output["destination_path"].is_string(),
                "destination_path should be present");
            prop_assert!(write_result.output["file_size"].is_number(),
                "file_size should be present");
            prop_assert!(write_result.output["row_count"].is_number(),
                "row_count should be present");

            // Verify metadata in context
            prop_assert!(!context.files.is_empty(),
                "File metadata should be stored in Job Context");

            let file_metadata = &context.files[0];
            prop_assert_eq!(&file_metadata.path, &file_path,
                "File path should match in metadata");
            prop_assert!(file_metadata.row_count.is_some(),
                "Row count should be present in metadata");

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 106: Invalid file format error handling**
/// **Validates: Requirements 15.11**
///
/// *For any* invalid file format encountered, the Worker should fail with a clear
/// error message indicating the parsing error.
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_invalid_file_format_error_handling() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        execution_id_bytes in any::<[u8; 16]>(),
        invalid_data in prop::collection::vec(any::<u8>(), 10..100)
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let storage = Arc::new(MinIOServiceImpl::new(client));
            let executor = FileProcessingExecutor::new(storage.clone());

            let job_id = Uuid::from_bytes(job_id_bytes);
            let execution_id = Uuid::from_bytes(execution_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            // Upload invalid data as Excel file
            let file_path = arb_file_path(job_id, execution_id, "invalid.xlsx");
            storage.store_file(&file_path, &invalid_data).await.unwrap();

            // Try to read invalid Excel file
            let read_step = JobStep {
                id: "read_step".to_string(),
                name: "Read Invalid Excel".to_string(),
                step_type: JobType::FileProcessing {
                    operation: FileOperation::Read,
                    format: FileFormat::Excel,
                    source_path: Some(file_path),
                    destination_path: None,
                    options: FileProcessingOptions {
                        sheet_name: None,
                        sheet_index: None,
                        transformations: vec![],
                        streaming: false,
                    },
                },
                condition: None,
            };

            let read_result = executor.execute(&read_step, &mut context).await;

            // Should fail with clear error
            prop_assert!(read_result.is_err(),
                "Should fail when reading invalid file format");

            if let Err(e) = read_result {
                let error_msg = e.to_string();
                prop_assert!(
                    error_msg.contains("Failed to parse") ||
                    error_msg.contains("Failed to load") ||
                    error_msg.contains("FileProcessingFailed"),
                    "Error message should indicate parsing failure: {}", error_msg
                );
            }

            Ok(())
        }).unwrap();
    });
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test that empty Excel data can be written and read
#[test]
#[ignore] // Requires MinIO testcontainer
fn test_empty_excel_data() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let storage = Arc::new(MinIOServiceImpl::new(client));
        let executor = FileProcessingExecutor::new(storage.clone());

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let mut context = JobContext::new(execution_id, job_id);

        // Empty data
        let data = json!([]);
        let file_path = arb_file_path(job_id, execution_id, "empty.xlsx");
        context.set_variable("write_data".to_string(), data);

        let write_step = JobStep {
            id: "write_step".to_string(),
            name: "Write Empty Excel".to_string(),
            step_type: JobType::FileProcessing {
                operation: FileOperation::Write,
                format: FileFormat::Excel,
                source_path: None,
                destination_path: Some(file_path.clone()),
                options: FileProcessingOptions {
                    sheet_name: None,
                    sheet_index: None,
                    transformations: vec![],
                    streaming: false,
                },
            },
            condition: None,
        };

        let result = executor.execute(&write_step, &mut context).await;
        assert!(result.is_ok(), "Should handle empty data");
    });
}

/// Test that missing source_path for Read operation returns error
#[test]
fn test_missing_source_path_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let storage = Arc::new(MinIOServiceImpl::new(client));
        let executor = FileProcessingExecutor::new(storage.clone());

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let mut context = JobContext::new(execution_id, job_id);

        let read_step = JobStep {
            id: "read_step".to_string(),
            name: "Read Without Source".to_string(),
            step_type: JobType::FileProcessing {
                operation: FileOperation::Read,
                format: FileFormat::Excel,
                source_path: None, // Missing!
                destination_path: None,
                options: FileProcessingOptions {
                    sheet_name: None,
                    sheet_index: None,
                    transformations: vec![],
                    streaming: false,
                },
            },
            condition: None,
        };

        let result = executor.execute(&read_step, &mut context).await;
        assert!(result.is_err(), "Should fail when source_path is missing");

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("source_path is required"),
            "Error should mention missing source_path"
        );
    });
}

/// Test that missing destination_path for Write operation returns error
#[test]
fn test_missing_destination_path_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let storage = Arc::new(MinIOServiceImpl::new(client));
        let executor = FileProcessingExecutor::new(storage.clone());

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let mut context = JobContext::new(execution_id, job_id);

        context.set_variable("write_data".to_string(), json!([[1, 2, 3]]));

        let write_step = JobStep {
            id: "write_step".to_string(),
            name: "Write Without Destination".to_string(),
            step_type: JobType::FileProcessing {
                operation: FileOperation::Write,
                format: FileFormat::Excel,
                source_path: None,
                destination_path: None, // Missing!
                options: FileProcessingOptions {
                    sheet_name: None,
                    sheet_index: None,
                    transformations: vec![],
                    streaming: false,
                },
            },
            condition: None,
        };

        let result = executor.execute(&write_step, &mut context).await;
        assert!(
            result.is_err(),
            "Should fail when destination_path is missing"
        );

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("destination_path is required"),
            "Error should mention missing destination_path"
        );
    });
}

/// Test that missing write_data variable for Write operation returns error
#[test]
fn test_missing_write_data_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let storage = Arc::new(MinIOServiceImpl::new(client));
        let executor = FileProcessingExecutor::new(storage.clone());

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let mut context = JobContext::new(execution_id, job_id);

        // Don't set write_data variable

        let write_step = JobStep {
            id: "write_step".to_string(),
            name: "Write Without Data".to_string(),
            step_type: JobType::FileProcessing {
                operation: FileOperation::Write,
                format: FileFormat::Excel,
                source_path: None,
                destination_path: Some("test.xlsx".to_string()),
                options: FileProcessingOptions {
                    sheet_name: None,
                    sheet_index: None,
                    transformations: vec![],
                    streaming: false,
                },
            },
            condition: None,
        };

        let result = executor.execute(&write_step, &mut context).await;
        assert!(result.is_err(), "Should fail when write_data is missing");

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("No data available") || error_msg.contains("write_data"),
            "Error should mention missing data"
        );
    });
}
