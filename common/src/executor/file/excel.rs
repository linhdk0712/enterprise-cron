// Excel file processor
// Requirements: 15.1, 15.2, 15.5, 15.7 - Read/write XLSX files

use crate::errors::ExecutionError;
use crate::models::{FileMetadata, FileProcessingOptions, JobContext};
use crate::storage::StorageService;
use calamine::{open_workbook_auto_from_rs, Reader, Sheets};
use chrono::Utc;
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use serde_json::{json, Value};
use std::io::Cursor;
use std::sync::Arc;
use tracing::{info, instrument};

/// Excel file processor
pub struct ExcelProcessor {
    storage: Arc<dyn StorageService>,
}

impl ExcelProcessor {
    /// Create a new Excel processor
    pub fn new(storage: Arc<dyn StorageService>) -> Self {
        Self { storage }
    }

    /// Read Excel file from MinIO and parse to JSON
    #[instrument(skip(self, options))]
    pub async fn read(
        &self,
        source_path: &str,
        options: &FileProcessingOptions,
        _context: &JobContext,
    ) -> Result<Value, ExecutionError> {
        info!("Reading Excel file from: {}", source_path);

        // Load file from MinIO
        let file_data = self.storage.load_file(source_path).await.map_err(|e| {
            ExecutionError::FileProcessingFailed(format!("Failed to load file: {}", e))
        })?;

        // Parse Excel file
        let cursor = Cursor::new(file_data);
        let mut workbook: Sheets<_> = open_workbook_auto_from_rs(cursor).map_err(|e| {
            ExecutionError::FileProcessingFailed(format!("Failed to parse Excel file: {}", e))
        })?;

        // Determine which sheets to read
        let sheets_data = if let Some(sheet_name) = &options.sheet_name {
            let range = workbook.worksheet_range(sheet_name).map_err(|e| {
                ExecutionError::FileProcessingFailed(format!(
                    "Sheet '{}' not found: {}",
                    sheet_name, e
                ))
            })?;

            let sheet_json = self.parse_excel_range(&range)?;
            json!({ sheet_name: sheet_json })
        } else if let Some(sheet_index) = options.sheet_index {
            let sheet_names = workbook.sheet_names();
            if sheet_index >= sheet_names.len() {
                return Err(ExecutionError::FileProcessingFailed(format!(
                    "Sheet index {} out of bounds (total sheets: {})",
                    sheet_index,
                    sheet_names.len()
                )));
            }

            let sheet_name = &sheet_names[sheet_index];
            let range = workbook.worksheet_range(sheet_name).map_err(|e| {
                ExecutionError::FileProcessingFailed(format!("Failed to read sheet: {}", e))
            })?;

            let sheet_json = self.parse_excel_range(&range)?;
            json!({ sheet_name: sheet_json })
        } else {
            // Read all sheets
            let mut all_sheets = serde_json::Map::new();
            for sheet_name in workbook.sheet_names() {
                let range = workbook.worksheet_range(&sheet_name).map_err(|e| {
                    ExecutionError::FileProcessingFailed(format!("Failed to read sheet: {}", e))
                })?;

                let sheet_json = self.parse_excel_range(&range)?;
                all_sheets.insert(sheet_name.to_string(), sheet_json);
            }
            Value::Object(all_sheets)
        };

        Ok(sheets_data)
    }

    /// Parse Excel range to JSON array
    fn parse_excel_range(
        &self,
        range: &calamine::Range<calamine::Data>,
    ) -> Result<Value, ExecutionError> {
        let mut rows = Vec::new();

        for row in range.rows() {
            let mut row_data = Vec::new();
            for cell in row {
                let cell_value = match cell {
                    calamine::Data::Int(i) => json!(i),
                    calamine::Data::Float(f) => json!(f),
                    calamine::Data::String(s) => json!(s),
                    calamine::Data::Bool(b) => json!(b),
                    calamine::Data::DateTime(dt) => json!(dt.as_f64()),
                    calamine::Data::Error(e) => json!(format!("ERROR: {:?}", e)),
                    calamine::Data::Empty => Value::Null,
                    _ => Value::Null,
                };
                row_data.push(cell_value);
            }
            rows.push(Value::Array(row_data));
        }

        Ok(Value::Array(rows))
    }

    /// Write Excel file from JSON data
    #[instrument(skip(self, data))]
    pub async fn write(
        &self,
        data: &Value,
        destination_path: &str,
        _context: &JobContext,
    ) -> Result<FileMetadata, ExecutionError> {
        info!("Writing Excel file to: {}", destination_path);

        let mut workbook = Workbook::new();

        // Handle data structure
        match data {
            Value::Object(sheets) => {
                for (sheet_name, sheet_data) in sheets {
                    let worksheet = workbook.add_worksheet();
                    worksheet.set_name(sheet_name).map_err(|e| {
                        ExecutionError::FileProcessingFailed(format!("Invalid sheet name: {}", e))
                    })?;
                    self.write_excel_sheet(worksheet, sheet_data)?;
                }
            }
            Value::Array(_) => {
                let worksheet = workbook.add_worksheet();
                worksheet.set_name("Sheet1").map_err(|e| {
                    ExecutionError::FileProcessingFailed(format!("Failed to set sheet name: {}", e))
                })?;
                self.write_excel_sheet(worksheet, data)?;
            }
            _ => {
                return Err(ExecutionError::FileProcessingFailed(
                    "Invalid data format for Excel export".to_string(),
                ));
            }
        }

        // Save workbook to bytes
        let buffer = workbook.save_to_buffer().map_err(|e| {
            ExecutionError::FileProcessingFailed(format!("Failed to save Excel file: {}", e))
        })?;

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
        let row_count = self.count_rows(data);

        Ok(FileMetadata {
            path: destination_path.to_string(),
            filename: destination_path
                .split('/')
                .last()
                .unwrap_or("output.xlsx")
                .to_string(),
            size: file_size,
            mime_type: Some(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            ),
            row_count: Some(row_count),
            created_at: Utc::now(),
        })
    }

    /// Write data to Excel worksheet
    fn write_excel_sheet(
        &self,
        worksheet: &mut Worksheet,
        data: &Value,
    ) -> Result<(), ExecutionError> {
        if let Value::Array(rows) = data {
            for (row_idx, row) in rows.iter().enumerate() {
                if let Value::Array(cells) = row {
                    for (col_idx, cell) in cells.iter().enumerate() {
                        let row_num = row_idx as u32;
                        let col_num = col_idx as u16;

                        match cell {
                            Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    worksheet.write_number(row_num, col_num, i as f64).map_err(
                                        |e| {
                                            ExecutionError::FileProcessingFailed(format!(
                                                "Failed to write number: {}",
                                                e
                                            ))
                                        },
                                    )?;
                                } else if let Some(f) = n.as_f64() {
                                    worksheet.write_number(row_num, col_num, f).map_err(|e| {
                                        ExecutionError::FileProcessingFailed(format!(
                                            "Failed to write number: {}",
                                            e
                                        ))
                                    })?;
                                }
                            }
                            Value::String(s) => {
                                worksheet.write_string(row_num, col_num, s).map_err(|e| {
                                    ExecutionError::FileProcessingFailed(format!(
                                        "Failed to write string: {}",
                                        e
                                    ))
                                })?;
                            }
                            Value::Bool(b) => {
                                worksheet.write_boolean(row_num, col_num, *b).map_err(|e| {
                                    ExecutionError::FileProcessingFailed(format!(
                                        "Failed to write boolean: {}",
                                        e
                                    ))
                                })?;
                            }
                            Value::Null => {
                                worksheet
                                    .write_blank(row_num, col_num, &Format::new())
                                    .map_err(|e| {
                                        ExecutionError::FileProcessingFailed(format!(
                                            "Failed to write blank: {}",
                                            e
                                        ))
                                    })?;
                            }
                            _ => {
                                worksheet
                                    .write_string(row_num, col_num, &cell.to_string())
                                    .map_err(|e| {
                                        ExecutionError::FileProcessingFailed(format!(
                                            "Failed to write value: {}",
                                            e
                                        ))
                                    })?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Count rows in data
    fn count_rows(&self, data: &Value) -> usize {
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
