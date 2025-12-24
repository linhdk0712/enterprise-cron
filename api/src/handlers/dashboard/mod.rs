// Dashboard handlers module
// Tách theo RECC 2025 rules - File naming & module organization

mod executions_list;
mod job_details;
mod job_form;
mod jobs_list;
mod shared_utils;
mod stats;
mod variables_list;

pub use executions_list::executions_partial;
pub use job_details::{job_details_modal, job_details_partial};
pub use job_form::job_form_page;
pub use jobs_list::jobs_partial;
pub use stats::dashboard_index;
pub use variables_list::variables_partial;

// Re-export shared utilities for use within dashboard module

// Shared types
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ExecutionQueryParams {
    pub job_id: Option<Uuid>,
    pub status: Option<String>,
    pub trigger_source: Option<String>,
    pub job_name: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}
