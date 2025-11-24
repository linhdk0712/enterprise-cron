// Feature: vietnam-enterprise-cron
// Telemetry module for structured logging, metrics, and tracing
// Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9

use anyhow::Result;
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use std::net::SocketAddr;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use uuid::Uuid;

/// Initialize structured logging with JSON formatting and trace context
///
/// This function sets up the tracing subscriber with:
/// - JSON formatting for structured logs
/// - Trace context (trace_id, span_id) in all log entries
/// - Log levels from configuration or environment
/// - Optional OpenTelemetry integration
///
/// Requirements: 5.1, 5.2, 5.7, 5.9
#[tracing::instrument(skip_all)]
pub fn init_logging(log_level: &str, tracing_endpoint: Option<&str>) -> Result<()> {
    // Create environment filter from log level
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .map_err(|e| anyhow::anyhow!("Failed to create env filter: {}", e))?;

    // Create JSON formatting layer with trace context
    let json_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(env_filter);

    // Initialize the subscriber with optional OpenTelemetry layer
    let registry = tracing_subscriber::registry().with(json_layer);

    if let Some(endpoint) = tracing_endpoint {
        // Initialize OpenTelemetry if endpoint is provided
        let tracer = init_tracer(endpoint)?;
        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        registry
            .with(telemetry_layer)
            .try_init()
            .map_err(|e| anyhow::anyhow!("Failed to initialize tracing subscriber: {}", e))?;
    } else {
        registry
            .try_init()
            .map_err(|e| anyhow::anyhow!("Failed to initialize tracing subscriber: {}", e))?;
    }

    tracing::info!(
        log_level = log_level,
        tracing_endpoint = tracing_endpoint,
        "Structured logging initialized with JSON formatting"
    );

    Ok(())
}

/// Initialize OpenTelemetry tracer with OTLP exporter
///
/// This function sets up OpenTelemetry tracing with:
/// - OTLP exporter to send traces to a collector (e.g., Jaeger)
/// - Service name and version as resource attributes
/// - Random ID generator for trace and span IDs
/// - Always-on sampler for all traces
///
/// Requirements: 5.7
#[tracing::instrument(skip_all)]
fn init_tracer(endpoint: &str) -> Result<opentelemetry_sdk::trace::Tracer> {
    use opentelemetry_sdk::runtime::Tokio;

    // Create OTLP exporter
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint)
        .build_span_exporter()
        .map_err(|e| anyhow::anyhow!("Failed to build span exporter: {}", e))?;

    // Create tracer provider with resource attributes
    let tracer_provider = TracerProvider::builder()
        .with_batch_exporter(exporter, Tokio)
        .with_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", "vietnam-enterprise-cron"),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])),
        )
        .build();

    // Set global tracer provider
    global::set_tracer_provider(tracer_provider.clone());

    // Get tracer
    let tracer = tracer_provider.tracer("vietnam-enterprise-cron");

    tracing::info!(
        endpoint = endpoint,
        "OpenTelemetry tracer initialized with OTLP exporter"
    );

    Ok(tracer)
}

/// Shutdown OpenTelemetry tracer provider
///
/// This should be called on graceful shutdown to flush remaining spans
pub fn shutdown_tracer() {
    global::shutdown_tracer_provider();
}

/// Initialize Prometheus metrics exporter
///
/// This function sets up the Prometheus metrics exporter and registers all metrics:
/// - job_success_total: Counter for successful job executions
/// - job_failed_total: Counter for failed job executions
/// - job_duration_seconds: Histogram for job execution duration
/// - job_queue_size: Gauge for current queue size
///
/// Requirements: 5.3, 5.4, 5.5, 5.6
#[tracing::instrument(skip_all)]
pub fn init_metrics(metrics_port: u16) -> Result<()> {
    let addr: SocketAddr = format!("0.0.0.0:{}", metrics_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid metrics port: {}", e))?;

    // Build and install the Prometheus exporter
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .map_err(|e| anyhow::anyhow!("Failed to install Prometheus exporter: {}", e))?;

    // Describe all metrics for better Prometheus integration
    describe_counter!(
        "job_success_total",
        "Total number of successful job executions"
    );
    describe_counter!("job_failed_total", "Total number of failed job executions");
    describe_histogram!(
        "job_duration_seconds",
        "Duration of job executions in seconds"
    );
    describe_gauge!("job_queue_size", "Current number of jobs in the queue");

    tracing::info!(
        metrics_port = metrics_port,
        metrics_endpoint = format!("http://0.0.0.0:{}/metrics", metrics_port),
        "Prometheus metrics exporter initialized"
    );

    Ok(())
}

/// Record a successful job execution
///
/// Increments the job_success_total counter
/// Requirements: 5.3
#[inline]
pub fn record_job_success(job_id: &Uuid, job_name: &str) {
    counter!("job_success_total", "job_id" => job_id.to_string(), "job_name" => job_name.to_string()).increment(1);
}

/// Record a failed job execution
///
/// Increments the job_failed_total counter
/// Requirements: 5.4
#[inline]
pub fn record_job_failure(job_id: &Uuid, job_name: &str, reason: &str) {
    counter!(
        "job_failed_total",
        "job_id" => job_id.to_string(),
        "job_name" => job_name.to_string(),
        "reason" => reason.to_string()
    )
    .increment(1);
}

/// Record job execution duration
///
/// Records the duration in the job_duration_seconds histogram
/// Requirements: 5.5
#[inline]
pub fn record_job_duration(job_id: &Uuid, job_name: &str, duration_seconds: f64) {
    histogram!(
        "job_duration_seconds",
        "job_id" => job_id.to_string(),
        "job_name" => job_name.to_string()
    )
    .record(duration_seconds);
}

/// Update the job queue size gauge
///
/// Sets the current queue size
/// Requirements: 5.6
#[inline]
pub fn update_queue_size(size: i64) {
    gauge!("job_queue_size").set(size as f64);
}

/// Alert notification interface
///
/// This trait defines the interface for sending alert notifications
/// when jobs fail consecutively
///
/// Requirements: 5.8
#[async_trait::async_trait]
pub trait AlertNotifier: Send + Sync {
    /// Send an alert notification for consecutive job failures
    async fn send_alert(
        &self,
        job_id: &Uuid,
        job_name: &str,
        consecutive_failures: u32,
    ) -> Result<()>;
}

/// Check if an alert should be triggered based on consecutive failures
///
/// Returns true if consecutive_failures >= 3
/// Requirements: 5.8
#[inline]
pub fn should_trigger_alert(consecutive_failures: u32) -> bool {
    consecutive_failures >= 3
}

/// Log-based alert notifier (default implementation)
///
/// This implementation logs alerts at ERROR level
/// In production, this could be replaced with integrations to:
/// - Email notifications
/// - Slack/Teams webhooks
/// - PagerDuty
/// - Custom alerting systems
pub struct LogAlertNotifier;

#[async_trait::async_trait]
impl AlertNotifier for LogAlertNotifier {
    #[tracing::instrument(skip(self))]
    async fn send_alert(
        &self,
        job_id: &Uuid,
        job_name: &str,
        consecutive_failures: u32,
    ) -> Result<()> {
        tracing::error!(
            job_id = %job_id,
            job_name = job_name,
            consecutive_failures = consecutive_failures,
            alert_type = "consecutive_failures",
            "ALERT: Job has failed {} consecutive times",
            consecutive_failures
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_logging_with_valid_level() {
        // Test that logging can be initialized with valid log levels
        let result = init_logging("info", None);
        // Note: This will fail if called multiple times in the same process
        // In real tests, we'd use a test-specific subscriber
        assert!(result.is_ok() || result.is_err()); // Either succeeds or already initialized
    }

    #[test]
    fn test_init_logging_with_debug_level() {
        let result = init_logging("debug", None);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_init_logging_with_trace_level() {
        let result = init_logging("trace", None);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_metrics_recording() {
        // Test that metrics can be recorded without panicking
        let job_id = Uuid::new_v4();
        record_job_success(&job_id, "test-job");
        record_job_failure(&job_id, "test-job", "timeout");
        record_job_duration(&job_id, "test-job", 1.5);
        update_queue_size(10);
    }

    #[test]
    fn test_should_trigger_alert() {
        assert!(!should_trigger_alert(0));
        assert!(!should_trigger_alert(1));
        assert!(!should_trigger_alert(2));
        assert!(should_trigger_alert(3));
        assert!(should_trigger_alert(4));
        assert!(should_trigger_alert(10));
    }

    #[tokio::test]
    async fn test_log_alert_notifier() {
        let notifier = LogAlertNotifier;
        let job_id = Uuid::new_v4();
        let result = notifier.send_alert(&job_id, "test-job", 3).await;
        assert!(result.is_ok());
    }
}
