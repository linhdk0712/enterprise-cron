// CSV file processor
// Requirements: 15.3, 15.4, 15.8 - Read/write CSV files with configurable delimiters

use crate::errors::ExecutionError;
use crate::models::{FileMetadata, FileProcessingOptions, JobContext};
use crate::storage::service::MinIOService;
use chrono::Utc;
use csv::{ReaderBuilder, WriterBuilder};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, instrument};

/// CSV file processor
pub struct CsvProcessor {
    storage: Arc<dyn MinIOService>,
}

impl CsvProcessor {
    /// Create a new CSV processor
    pub fn new(storage: Arc<dyn MinIOService>) -> Self {
        Self { storage }
    }

    /// Read CSV file from MinIO and parse to JSON
    #[instrument(skip(self, options))]
    pub async fn read(
        &self,
        source_path: &str,
        delimiter: char,
        _options: &FileProcessingOptions,
        _context: &JobContext,
    ) -> Result<Value, ExecutionError> {
        info!(
            "Reading CSV file from: {} with delimiter: {:?}",
            source_path, delimiter
        );

        // Load file from MinIO
        let file_data = self.storage.load_file(source_path).await.map_err(|e| {
            ExecutionError::FileProcessingFailed(format!("Failed to load file: {}", e))
        })?;

        // Parse CSV file
        let mut reader = ReaderBuilder::new()
            .delimiter(delimiter as u8)
            .from_reader(file_data.as_slice());

        let mut rows = Vec::new();

        // Read all records
        for result in reader.records() {
            let record = result.map_err(|e| {
                ExecutionError::FileProcessingFailed(format!("Failed to parse CSV record: {}", e))
            })?;

            let mut row_data = Vec::new();
            for field in record.iter() {
                // Try to parse as number, otherwise keep as string
                if let Ok(num) = field.parse::<f64>() {
                    row_data.push(json!(num));
                } else if let Ok(b) = field.parse::<bool>() {
                    row_data.push(json!(b));
                } else if field.is_empty() {
                    row_data.push(Value::Null);
                } else {
                    row_data.push(json!(field));
                }
            }
            rows.push(Value::Array(row_data));
        }

        Ok(Value::Array(rows))
    }

    /// Write CSV file from JSON data
    #[instrument(skip(self, data))]
    pub async fn write(
        &self,
        data: &Value,
        destination_path: &str,
        delimiter: char,
        _context: &JobContext,
    ) -> Result<FileMetadata, ExecutionError> {
        info!(
            "Writing CSV file to: {} with delimiter: {:?}",
            destination_path, delimiter
        );

        let mut buffer = Vec::new();
        let mut writer = WriterBuilder::new()
            .delimiter(delimiter as u8)
            .from_writer(&mut buffer);

        // Write data rows
        if let Value::Array(rows) = data {
            for row in rows {
                if let Value::Array(cells) = row {
                    let string_cells: Vec<String> = cells
                        .iter()
                        .map(|cell| match cell {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            Value::Null => String::new(),
                            _ => cell.to_string(),
                        })
                        .collect();

                    writer.write_record(&string_cells).map_err(|e| {
                        ExecutionError::FileProcessingFailed(format!(
                            "Failed to write CSV record: {}",
                            e
                        ))
                    })?;
                }
            }
        } else {
            return Err(ExecutionError::FileProcessingFailed(
                "Invalid data format for CSV export. Expected array of arrays.".to_string(),
            ));
        }

        writer.flush().map_err(|e| {
            ExecutionError::FileProcessingFailed(format!("Failed to flush CSV writer: {}", e))
        })?;

        drop(writer);

        // Upload to MinIO
        let file_size = buffer.len() as u64;
        self.storage
            .store_file(destination_path, &buffer)
            .await
            .map_err(|e| {
                ExecutionError::FileProcessingFailed(format!(
                    "Failed to upload file to MinIO: {}",
                    e
                ))
            })?;

        // Count rows
        let row_count = if let Value::Array(rows) = data {
            rows.len()
        } else {
            0
        };

        Ok(FileMetadata {
            path: destination_path.to_string(),
            filename: destination_path
                .split('/')
                .last()
                .unwrap_or("output.csv")
                .to_string(),
            size: file_size,
            mime_type: Some("text/csv".to_string()),
            row_count: Some(row_count),
            created_at: Utc::now(),
        })
    }
}
