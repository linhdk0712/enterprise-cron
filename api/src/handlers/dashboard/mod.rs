// Dashboard handlers module
// Tách theo RECC 2025 rules - File naming & module organization

mod stats;
mod jobs_list;
mod job_details;
mod executions_list;
mod variables_list;
mod job_form;
mod shared_utils;

pub use stats::dashboard_index;
pub use jobs_list::jobs_partial;
pub use job_details::{job_details_modal, job_details_partial};
pub use executions_list::executions_partial;
pub use variables_list::variables_partial;
pub use job_form::job_form_page;

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
