// Schedule parsing and calculation module
//
// This module implements schedule parsing and next execution time calculation
// for all schedule types: Cron, FixedDelay, FixedRate, and OneTime.
//
// Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7

use crate::errors::ScheduleError;
use crate::models::Schedule;
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use cron::Schedule as CronSchedule;
use std::str::FromStr;

/// ScheduleTrigger trait defines the interface for calculating next execution times
pub trait ScheduleTrigger {
    /// Calculate the next execution time based on the schedule and last execution time
    fn next_execution_time(
        &self,
        last_execution: Option<DateTime<Utc>>,
    ) -> Result<Option<DateTime<Utc>>, ScheduleError>;

    /// Check if the schedule has completed (for one-time jobs or jobs with end dates)
    fn is_complete(&self, last_execution: Option<DateTime<Utc>>) -> bool;
}

impl ScheduleTrigger for Schedule {
    fn next_execution_time(
        &self,
        last_execution: Option<DateTime<Utc>>,
    ) -> Result<Option<DateTime<Utc>>, ScheduleError> {
        match self {
            Schedule::Cron {
                expression,
                timezone,
                end_date,
            } => calculate_cron_next_execution(expression, *timezone, *end_date, last_execution),

            Schedule::FixedDelay { delay_seconds } => {
                calculate_fixed_delay_next_execution(*delay_seconds, last_execution)
            }

            Schedule::FixedRate { interval_seconds } => {
                calculate_fixed_rate_next_execution(*interval_seconds, last_execution)
            }

            Schedule::OneTime { execute_at } => {
                calculate_one_time_next_execution(*execute_at, last_execution)
            }
        }
    }

    fn is_complete(&self, last_execution: Option<DateTime<Utc>>) -> bool {
        match self {
            Schedule::Cron { end_date, .. } => {
                // Cron is complete if there's an end date and we've passed it
                if let Some(end) = end_date {
                    if let Some(last) = last_execution {
                        return last >= *end;
                    }
                }
                false
            }

            Schedule::OneTime { .. } => {
                // One-time jobs are complete after first execution
                last_execution.is_some()
            }

            // Fixed delay and fixed rate jobs never complete
            Schedule::FixedDelay { .. } | Schedule::FixedRate { .. } => false,
        }
    }
}

/// Parse and validate a cron expression
///
/// Requirements: 1.1 - Parse Quartz syntax with second precision
pub fn parse_cron_expression(expression: &str) -> Result<CronSchedule, ScheduleError> {
    CronSchedule::from_str(expression).map_err(|e| ScheduleError::InvalidCronExpression {
        expression: expression.to_string(),
        reason: e.to_string(),
    })
}

/// Calculate next execution time for cron schedules
///
/// Requirements:
/// - 1.1: Parse cron expression using Quartz syntax with second precision
/// - 1.2: Evaluate cron expression in specified timezone
/// - 1.7: Stop scheduling after end date
fn calculate_cron_next_execution(
    expression: &str,
    timezone: Tz,
    end_date: Option<DateTime<Utc>>,
    last_execution: Option<DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>, ScheduleError> {
    // Parse the cron expression
    let schedule = parse_cron_expression(expression)?;

    // Determine the reference time (last execution or now)
    let reference_time = last_execution.unwrap_or_else(Utc::now);

    // Convert to the job's timezone
    let reference_in_tz = reference_time.with_timezone(&timezone);

    // Find the next execution time in the job's timezone
    let next_in_tz =
        schedule
            .after(&reference_in_tz)
            .next()
            .ok_or_else(|| ScheduleError::NoNextExecution {
                schedule_type: "cron".to_string(),
            })?;

    // Convert back to UTC
    let next_utc = next_in_tz.with_timezone(&Utc);

    // Check if we've passed the end date
    if let Some(end) = end_date {
        if next_utc > end {
            return Ok(None);
        }
    }

    Ok(Some(next_utc))
}

/// Calculate next execution time for fixed delay schedules
///
/// Requirements: 1.4 - Schedule next execution X seconds after previous completion
fn calculate_fixed_delay_next_execution(
    delay_seconds: u32,
    last_execution: Option<DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>, ScheduleError> {
    match last_execution {
        Some(last) => {
            // For fixed delay, next execution is delay_seconds after the last execution completed
            let delay = Duration::seconds(delay_seconds as i64);
            Ok(Some(last + delay))
        }
        None => {
            // First execution: schedule immediately
            Ok(Some(Utc::now()))
        }
    }
}

/// Calculate next execution time for fixed rate schedules
///
/// Requirements: 1.5 - Schedule executions at fixed intervals regardless of duration
fn calculate_fixed_rate_next_execution(
    interval_seconds: u32,
    last_execution: Option<DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>, ScheduleError> {
    match last_execution {
        Some(last) => {
            // For fixed rate, next execution is interval_seconds after the last execution started
            let interval = Duration::seconds(interval_seconds as i64);
            Ok(Some(last + interval))
        }
        None => {
            // First execution: schedule immediately
            Ok(Some(Utc::now()))
        }
    }
}

/// Calculate next execution time for one-time schedules
///
/// Requirements: 1.6 - Execute once at specific datetime and mark complete
fn calculate_one_time_next_execution(
    execute_at: DateTime<Utc>,
    last_execution: Option<DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>, ScheduleError> {
    if last_execution.is_some() {
        // Already executed, no next execution
        Ok(None)
    } else {
        // Not yet executed, return the scheduled time
        Ok(Some(execute_at))
    }
}

/// Get the default timezone for jobs
///
/// Requirements: 1.3 - Default to Asia/Ho_Chi_Minh timezone
pub fn default_timezone() -> Tz {
    chrono_tz::Asia::Ho_Chi_Minh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_cron_expression() {
        // Valid cron expression with second precision
        let result = parse_cron_expression("0 0 12 * * * *");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_cron_expression() {
        // Invalid cron expression
        let result = parse_cron_expression("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_default_timezone() {
        let tz = default_timezone();
        assert_eq!(tz.to_string(), "Asia/Ho_Chi_Minh");
    }

    #[test]
    fn test_fixed_delay_first_execution() {
        let schedule = Schedule::FixedDelay { delay_seconds: 60 };
        let next = schedule.next_execution_time(None).unwrap();
        assert!(next.is_some());
    }

    #[test]
    fn test_fixed_delay_subsequent_execution() {
        let schedule = Schedule::FixedDelay { delay_seconds: 60 };
        let last = Utc::now();
        let next = schedule.next_execution_time(Some(last)).unwrap().unwrap();
        let expected = last + Duration::seconds(60);
        assert!((next - expected).num_seconds().abs() < 1);
    }

    #[test]
    fn test_fixed_rate_first_execution() {
        let schedule = Schedule::FixedRate {
            interval_seconds: 60,
        };
        let next = schedule.next_execution_time(None).unwrap();
        assert!(next.is_some());
    }

    #[test]
    fn test_fixed_rate_subsequent_execution() {
        let schedule = Schedule::FixedRate {
            interval_seconds: 60,
        };
        let last = Utc::now();
        let next = schedule.next_execution_time(Some(last)).unwrap().unwrap();
        let expected = last + Duration::seconds(60);
        assert!((next - expected).num_seconds().abs() < 1);
    }

    #[test]
    fn test_one_time_not_executed() {
        let execute_at = Utc::now() + Duration::hours(1);
        let schedule = Schedule::OneTime { execute_at };
        let next = schedule.next_execution_time(None).unwrap();
        assert_eq!(next, Some(execute_at));
    }

    #[test]
    fn test_one_time_already_executed() {
        let execute_at = Utc::now() + Duration::hours(1);
        let schedule = Schedule::OneTime { execute_at };
        let last = Utc::now();
        let next = schedule.next_execution_time(Some(last)).unwrap();
        assert_eq!(next, None);
    }

    #[test]
    fn test_one_time_is_complete() {
        let execute_at = Utc::now() + Duration::hours(1);
        let schedule = Schedule::OneTime { execute_at };
        assert!(!schedule.is_complete(None));
        assert!(schedule.is_complete(Some(Utc::now())));
    }

    #[test]
    fn test_cron_with_end_date() {
        let schedule = Schedule::Cron {
            expression: "0 0 12 * * * *".to_string(),
            timezone: default_timezone(),
            end_date: Some(Utc::now() - Duration::days(1)), // End date in the past
        };
        let next = schedule.next_execution_time(None).unwrap();
        // Should return None because end date has passed
        assert_eq!(next, None);
    }

    #[test]
    fn test_cron_is_complete_with_end_date() {
        let end_date = Utc::now() - Duration::days(1);
        let schedule = Schedule::Cron {
            expression: "0 0 12 * * * *".to_string(),
            timezone: default_timezone(),
            end_date: Some(end_date),
        };
        let last_execution = Utc::now();
        assert!(schedule.is_complete(Some(last_execution)));
    }

    #[test]
    fn test_fixed_delay_never_complete() {
        let schedule = Schedule::FixedDelay { delay_seconds: 60 };
        assert!(!schedule.is_complete(None));
        assert!(!schedule.is_complete(Some(Utc::now())));
    }

    #[test]
    fn test_fixed_rate_never_complete() {
        let schedule = Schedule::FixedRate {
            interval_seconds: 60,
        };
        assert!(!schedule.is_complete(None));
        assert!(!schedule.is_complete(Some(Utc::now())));
    }
}
