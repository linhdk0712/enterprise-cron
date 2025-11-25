// Reference resolver for variables and step outputs
// Requirements: 14.1, 14.2, 14.4

use crate::models::JobContext;
use regex::Regex;
use std::sync::OnceLock;
use tracing::{instrument, warn};

/// Reference resolver for resolving variable and step output references
pub struct ReferenceResolver {
    // Regex pattern for matching references like {{variable}} or {{steps.step1.output}}
    pattern: OnceLock<Regex>,
}

impl ReferenceResolver {
    /// Create a new reference resolver
    pub fn new() -> Self {
        Self {
            pattern: OnceLock::new(),
        }
    }

    /// Get the regex pattern for matching references
    fn get_pattern(&self) -> &Regex {
        self.pattern.get_or_init(|| {
            // Match {{...}} patterns
            Regex::new(r"\{\{([^}]+)\}\}").expect("Invalid regex pattern")
        })
    }

    /// Resolve all references in a template string
    #[instrument(skip(self, context))]
    pub fn resolve(&self, template: &str, context: &JobContext) -> Result<String, String> {
        let pattern = self.get_pattern();
        let mut result = template.to_string();
        let mut errors = Vec::new();

        // Find all matches
        for cap in pattern.captures_iter(template) {
            if let Some(reference) = cap.get(1) {
                let ref_str = reference.as_str().trim();

                // Resolve the reference
                match self.resolve_reference(ref_str, context) {
                    Ok(value) => {
                        // Replace the reference with the resolved value
                        let full_match = cap.get(0).unwrap().as_str();
                        result = result.replace(full_match, &value);
                    }
                    Err(e) => {
                        errors.push(format!("Failed to resolve '{}': {}", ref_str, e));
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors.join("; "));
        }

        Ok(result)
    }

    /// Resolve a single reference
    fn resolve_reference(&self, reference: &str, context: &JobContext) -> Result<String, String> {
        // Check if it's a step output reference (starts with "steps.")
        if reference.starts_with("steps.") {
            self.resolve_step_output(reference, context)
        }
        // Check if it's a webhook reference (starts with "webhook.")
        else if reference.starts_with("webhook.") {
            self.resolve_webhook_data(reference, context)
        }
        // Otherwise, treat as a variable reference
        else {
            self.resolve_variable(reference, context)
        }
    }

    /// Resolve a step output reference like "steps.step1.output.data.id"
    fn resolve_step_output(&self, reference: &str, context: &JobContext) -> Result<String, String> {
        // Parse the reference: steps.{step_id}.{path}
        let parts: Vec<&str> = reference.split('.').collect();

        if parts.len() < 3 {
            return Err(format!("Invalid step reference: {}", reference));
        }

        let step_id = parts[1];

        // Get the step output
        let step_output = context
            .steps
            .get(step_id)
            .ok_or_else(|| format!("Step '{}' not found in context", step_id))?;

        // Navigate the JSON path
        let path = &parts[2..];
        self.navigate_json_path(&step_output.output, path)
    }

    /// Resolve a webhook data reference like "webhook.payload.user_id"
    fn resolve_webhook_data(
        &self,
        reference: &str,
        context: &JobContext,
    ) -> Result<String, String> {
        let webhook_data = context
            .webhook
            .as_ref()
            .ok_or_else(|| "No webhook data in context".to_string())?;

        // Parse the reference: webhook.{type}.{path}
        let parts: Vec<&str> = reference.split('.').collect();

        if parts.len() < 2 {
            return Err(format!("Invalid webhook reference: {}", reference));
        }

        match parts[1] {
            "payload" => {
                let path = &parts[2..];
                self.navigate_json_path(&webhook_data.payload, path)
            }
            "query_params" => {
                if parts.len() < 3 {
                    return Err("Missing query parameter name".to_string());
                }
                let param_name = parts[2];
                webhook_data
                    .query_params
                    .get(param_name)
                    .cloned()
                    .ok_or_else(|| format!("Query parameter '{}' not found", param_name))
            }
            "headers" => {
                if parts.len() < 3 {
                    return Err("Missing header name".to_string());
                }
                let header_name = parts[2];
                webhook_data
                    .headers
                    .get(header_name)
                    .cloned()
                    .ok_or_else(|| format!("Header '{}' not found", header_name))
            }
            _ => Err(format!("Invalid webhook data type: {}", parts[1])),
        }
    }

    /// Resolve a variable reference
    fn resolve_variable(&self, reference: &str, context: &JobContext) -> Result<String, String> {
        context
            .variables
            .get(reference)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Variable '{}' not found in context", reference))
    }

    /// Navigate a JSON path and return the value as a string
    fn navigate_json_path(
        &self,
        value: &serde_json::Value,
        path: &[&str],
    ) -> Result<String, String> {
        let mut current = value;

        for part in path {
            // Check if it's an array index
            if let Ok(index) = part.parse::<usize>() {
                current = current
                    .get(index)
                    .ok_or_else(|| format!("Array index {} not found", index))?;
            } else {
                // Object key
                current = current
                    .get(part)
                    .ok_or_else(|| format!("Key '{}' not found", part))?;
            }
        }

        // Convert the final value to a string
        match current {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Null => Ok("null".to_string()),
            _ => Ok(current.to_string()),
        }
    }
}

impl Default for ReferenceResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{StepOutput, WebhookData};
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn test_resolve_simple_variable() {
        let resolver = ReferenceResolver::new();
        let mut context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        context
            .variables
            .insert("api_key".to_string(), serde_json::json!("secret123"));

        let template = "Authorization: Bearer {{api_key}}";
        let result = resolver.resolve(template, &context).unwrap();

        assert_eq!(result, "Authorization: Bearer secret123");
    }

    #[test]
    fn test_resolve_step_output() {
        let resolver = ReferenceResolver::new();
        let mut context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        let step_output = StepOutput {
            step_id: "step1".to_string(),
            status: "success".to_string(),
            output: serde_json::json!({
                "data": {
                    "id": 123,
                    "name": "test"
                }
            }),
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };

        context.steps.insert("step1".to_string(), step_output);

        let template = "User ID: {{steps.step1.data.id}}";
        let result = resolver.resolve(template, &context).unwrap();

        assert_eq!(result, "User ID: 123");
    }

    #[test]
    fn test_resolve_webhook_payload() {
        let resolver = ReferenceResolver::new();
        let context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: Some(WebhookData {
                payload: serde_json::json!({
                    "user_id": "user123",
                    "action": "create"
                }),
                query_params: HashMap::new(),
                headers: HashMap::new(),
            }),
            files: Vec::new(),
        };

        let template = "Processing action for user {{webhook.payload.user_id}}";
        let result = resolver.resolve(template, &context).unwrap();

        assert_eq!(result, "Processing action for user user123");
    }

    #[test]
    fn test_resolve_missing_variable() {
        let resolver = ReferenceResolver::new();
        let context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        let template = "Value: {{missing_var}}";
        let result = resolver.resolve(template, &context);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Variable 'missing_var' not found"));
    }

    #[test]
    fn test_resolve_multiple_references() {
        let resolver = ReferenceResolver::new();
        let mut context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        context
            .variables
            .insert("host".to_string(), serde_json::json!("api.example.com"));
        context
            .variables
            .insert("port".to_string(), serde_json::json!("443"));

        let template = "https://{{host}}:{{port}}/api/v1";
        let result = resolver.resolve(template, &context).unwrap();

        assert_eq!(result, "https://api.example.com:443/api/v1");
    }
}
