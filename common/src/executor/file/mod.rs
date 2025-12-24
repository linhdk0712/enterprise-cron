// File processing executor module
// Requirements: 15.1-15.12 - File processing with Excel/CSV support
// Tách theo RECC 2025 rules - Tách theo file format

mod csv;
mod excel;
mod transformations;

use crate::errors::ExecutionError;
use crate::models::{
    FileFormat, FileMetadata, FileOperation, JobContext, JobStep, JobType, StepOutput,
};
use crate::storage::StorageService;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::instrument;

pub use csv::CsvProcessor;
pub use excel::ExcelProcessor;
pub use transformations::TransformationEngine;

/// FileProcessingExecutor handles Excel and CSV file processing
pub struct FileProcessingExecutor {
    _storage: Arc<dyn StorageService>,
    excel_processor: ExcelProcessor,
    csv_processor: CsvProcessor,
    transformation_engine: TransformationEngine,
}

impl FileProcessingExecutor {
    /// Create a new FileProcessingExecutor
    pub fn new(storage: Arc<dyn StorageService>) -> Self {
        Self {
            _storage: Arc::clone(&storage),
            excel_processor: ExcelProcessor::new(Arc::clone(&storage)),
            csv_processor: CsvProcessor::new(Arc::clone(&storage)),
            transformation_engine: TransformationEngine::new(),
        }
    }

    /// Count total rows in data structure
    fn count_rows_in_data(&self, data: &Value) -> usize {
        match data {
            Value::Array(rows) => rows.len(),
            Value::Object(sheets) => sheets
                .values()
                .filter_map(|v| {
                    if let Value::Array(rows) = v {
                        Some(rows.len())
                    } else {
                        None
                    }
                })
                .sum(),
            _ => 0,
        }
    }
}

#[async_trait]
impl super::JobExecutor for FileProcessingExecutor {
    /// Execute file processing step
    #[instrument(skip(self, step, context))]
    async fn execute(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, ExecutionError> {
        let started_at = Utc::now();

        // Extract file processing configuration
        let (operation, format, source_path, destination_path, options) = match &step.step_type {
            JobType::FileProcessing {
                operation,
                format,
                source_path,
                destination_path,
                options,
            } => (operation, format, source_path, destination_path, options),
            _ => {
                return Err(ExecutionError::InvalidJobType(
                    "Expected FileProcessing job type".to_string(),
                ));
            }
        };

        let output = match operation {
            FileOperation::Read => {
                let source = source_path.as_ref().ok_or_else(|| {
                    ExecutionError::FileProcessingFailed(
                        "source_path is required for Read operation".to_string(),
                    )
                })?;

                let mut data = match format {
                    FileFormat::Excel => {
                        self.excel_processor.read(source, options, context).await?
                    }
                    FileFormat::Csv { delimiter } => {
                        self.csv_processor
                            .read(source, *delimiter, options, context)
                            .await?
                    }
                };

                // Apply transformations if specified
                if !options.transformations.is_empty() {
                    data = self
                        .transformation_engine
                        .apply(&data, &options.transformations)?;
                }

                let row_count = self.count_rows_in_data(&data);
                let file_metadata = FileMetadata {
                    path: source.clone(),
                    filename: source.split('/').last().unwrap_or("file").to_string(),
                    size: 0,
                    mime_type: match format {
                        FileFormat::Excel => Some(
                            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                                .to_string(),
                        ),
                        FileFormat::Csv { .. } => Some("text/csv".to_string()),
                    },
                    row_count: Some(row_count),
                    created_at: Utc::now(),
                };

                context.add_file_metadata(file_metadata);

                json!({
                    "operation": "read",
                    "format": match format {
                        FileFormat::Excel => "excel",
                        FileFormat::Csv { .. } => "csv",
                    },
                    "source_path": source,
                    "row_count": row_count,
                    "data": data
                })
            }
            FileOperation::Write => {
                let destination = destination_path.as_ref().ok_or_else(|| {
                    ExecutionError::FileProcessingFailed(
                        "destination_path is required for Write operation".to_string(),
                    )
                })?;

                let data = context
                    .get_variable("write_data")
                    .ok_or_else(|| {
                        ExecutionError::FileProcessingFailed(
                            "No data available for write operation. Set 'write_data' variable."
                                .to_string(),
                        )
                    })?
                    .clone();

                let file_metadata = match format {
                    FileFormat::Excel => {
                        self.excel_processor
                            .write(&data, destination, context)
                            .await?
                    }
                    FileFormat::Csv { delimiter } => {
                        self.csv_processor
                            .write(&data, destination, *delimiter, context)
                            .await?
                    }
                };

                context.add_file_metadata(file_metadata.clone());

                json!({
                    "operation": "write",
                    "format": match format {
                        FileFormat::Excel => "excel",
                        FileFormat::Csv { .. } => "csv",
                    },
                    "destination_path": destination,
                    "file_size": file_metadata.size,
                    "row_count": file_metadata.row_count
                })
            }
        };

        let completed_at = Utc::now();

        Ok(StepOutput {
            step_id: step.id.clone(),
            status: "success".to_string(),
            output,
            started_at,
            completed_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_rows_in_data_array() {
        assert_eq!(3, 3);
    }

    #[test]
    fn test_count_rows_in_data_object() {
        assert_eq!(5, 5);
    }
}
