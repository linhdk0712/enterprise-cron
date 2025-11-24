// Variable substitution engine
// Requirements: 2.3, 2.4, 2.5 - Variable resolution with precedence and error handling

pub mod database;
pub mod http;

use crate::errors::SubstitutionError;
use regex::Regex;
use std::collections::HashMap;
use tracing::instrument;

/// VariableSubstitutor handles template variable substitution
///
/// Supports ${VAR_NAME} syntax for variable placeholders
/// Implements precedence: job-specific > global
pub struct VariableSubstitutor {
    /// Compiled regex for finding variable placeholders
    placeholder_regex: Regex,
}

impl VariableSubstitutor {
    /// Create a new VariableSubstitutor
    pub fn new() -> Result<Self, SubstitutionError> {
        // Regex to match ${VAR_NAME} patterns
        // Captures the variable name inside ${}
        let placeholder_regex = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}")
            .map_err(|e| SubstitutionError::RegexError(e.to_string()))?;

        Ok(Self { placeholder_regex })
    }

    /// Substitute variables in a template string
    ///
    /// # Arguments
    /// * `template` - The template string containing ${VAR_NAME} placeholders
    /// * `variables` - HashMap of variable names to values (with precedence already applied)
    ///
    /// # Requirements
    /// - 2.3: Variable resolution
    /// - 2.4: Variable precedence (caller must provide merged variables)
    /// - 2.5: Undefined variable handling
    ///
    /// # Returns
    /// The template string with all variables substituted
    ///
    /// # Errors
    /// Returns SubstitutionError::UndefinedVariable if a referenced variable is not found
    #[instrument(skip(self, variables), fields(template_len = template.len(), var_count = variables.len()))]
    pub fn substitute(
        &self,
        template: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String, SubstitutionError> {
        let mut result = template.to_string();
        let mut undefined_vars = Vec::new();

        // Find all variable placeholders
        for cap in self.placeholder_regex.captures_iter(template) {
            let full_match = cap.get(0).unwrap().as_str();
            let var_name = cap.get(1).unwrap().as_str();

            // Look up the variable value
            match variables.get(var_name) {
                Some(value) => {
                    result = result.replace(full_match, value);
                    tracing::debug!(
                        variable = var_name,
                        value_len = value.len(),
                        "Substituted variable"
                    );
                }
                None => {
                    undefined_vars.push(var_name.to_string());
                }
            }
        }

        // If any variables were undefined, return an error
        if !undefined_vars.is_empty() {
            tracing::error!(
                undefined_variables = ?undefined_vars,
                template = template,
                "Undefined variables in template"
            );
            return Err(SubstitutionError::UndefinedVariable {
                variables: undefined_vars,
                template: template.to_string(),
            });
        }

        tracing::debug!(
            original_len = template.len(),
            result_len = result.len(),
            "Variable substitution completed"
        );

        Ok(result)
    }

    /// Extract all variable names from a template
    ///
    /// # Arguments
    /// * `template` - The template string to analyze
    ///
    /// # Returns
    /// A vector of unique variable names found in the template
    pub fn extract_variables(&self, template: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for cap in self.placeholder_regex.captures_iter(template) {
            let var_name = cap.get(1).unwrap().as_str().to_string();
            if seen.insert(var_name.clone()) {
                variables.push(var_name);
            }
        }

        variables
    }

    /// Check if a template contains any variable placeholders
    ///
    /// # Arguments
    /// * `template` - The template string to check
    ///
    /// # Returns
    /// true if the template contains at least one variable placeholder
    pub fn has_variables(&self, template: &str) -> bool {
        self.placeholder_regex.is_match(template)
    }
}

impl Default for VariableSubstitutor {
    fn default() -> Self {
        Self::new().expect("Failed to create VariableSubstitutor")
    }
}

/// Merge global and job-specific variables with proper precedence
///
/// # Arguments
/// * `global_vars` - Global variables
/// * `job_vars` - Job-specific variables
///
/// # Requirements
/// - 2.4: Variable precedence (job-specific > global)
///
/// # Returns
/// A merged HashMap where job-specific variables override global ones
pub fn merge_variables(
    global_vars: HashMap<String, String>,
    job_vars: HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = global_vars;
    merged.extend(job_vars);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_single_variable() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let mut vars = HashMap::new();
        vars.insert("API_KEY".to_string(), "secret123".to_string());

        let result = substitutor
            .substitute("Authorization: Bearer ${API_KEY}", &vars)
            .unwrap();
        assert_eq!(result, "Authorization: Bearer secret123");
    }

    #[test]
    fn test_substitute_multiple_variables() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let mut vars = HashMap::new();
        vars.insert("HOST".to_string(), "api.example.com".to_string());
        vars.insert("PORT".to_string(), "8080".to_string());
        vars.insert("PATH".to_string(), "/v1/users".to_string());

        let result = substitutor
            .substitute("https://${HOST}:${PORT}${PATH}", &vars)
            .unwrap();
        assert_eq!(result, "https://api.example.com:8080/v1/users");
    }

    #[test]
    fn test_substitute_same_variable_multiple_times() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let mut vars = HashMap::new();
        vars.insert("USER".to_string(), "admin".to_string());

        let result = substitutor
            .substitute("User: ${USER}, Created by: ${USER}", &vars)
            .unwrap();
        assert_eq!(result, "User: admin, Created by: admin");
    }

    #[test]
    fn test_substitute_undefined_variable() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let vars = HashMap::new();

        let result = substitutor.substitute("Value: ${UNDEFINED}", &vars);
        assert!(result.is_err());

        match result {
            Err(SubstitutionError::UndefinedVariable { variables, .. }) => {
                assert_eq!(variables, vec!["UNDEFINED"]);
            }
            _ => panic!("Expected UndefinedVariable error"),
        }
    }

    #[test]
    fn test_substitute_multiple_undefined_variables() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let vars = HashMap::new();

        let result = substitutor.substitute("${VAR1} and ${VAR2}", &vars);
        assert!(result.is_err());

        match result {
            Err(SubstitutionError::UndefinedVariable { variables, .. }) => {
                assert_eq!(variables.len(), 2);
                assert!(variables.contains(&"VAR1".to_string()));
                assert!(variables.contains(&"VAR2".to_string()));
            }
            _ => panic!("Expected UndefinedVariable error"),
        }
    }

    #[test]
    fn test_substitute_no_variables() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let vars = HashMap::new();

        let result = substitutor.substitute("No variables here", &vars).unwrap();
        assert_eq!(result, "No variables here");
    }

    #[test]
    fn test_extract_variables() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let template = "https://${HOST}:${PORT}${PATH}";

        let vars = substitutor.extract_variables(template);
        assert_eq!(vars.len(), 3);
        assert!(vars.contains(&"HOST".to_string()));
        assert!(vars.contains(&"PORT".to_string()));
        assert!(vars.contains(&"PATH".to_string()));
    }

    #[test]
    fn test_extract_variables_with_duplicates() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let template = "${USER} created by ${USER}";

        let vars = substitutor.extract_variables(template);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], "USER");
    }

    #[test]
    fn test_has_variables() {
        let substitutor = VariableSubstitutor::new().unwrap();

        assert!(substitutor.has_variables("${VAR}"));
        assert!(substitutor.has_variables("text ${VAR} more text"));
        assert!(!substitutor.has_variables("no variables"));
        assert!(!substitutor.has_variables("$VAR without braces"));
    }

    #[test]
    fn test_merge_variables_precedence() {
        let mut global = HashMap::new();
        global.insert("VAR1".to_string(), "global1".to_string());
        global.insert("VAR2".to_string(), "global2".to_string());

        let mut job = HashMap::new();
        job.insert("VAR2".to_string(), "job2".to_string());
        job.insert("VAR3".to_string(), "job3".to_string());

        let merged = merge_variables(global, job);

        assert_eq!(merged.get("VAR1").unwrap(), "global1");
        assert_eq!(merged.get("VAR2").unwrap(), "job2"); // Job-specific overrides global
        assert_eq!(merged.get("VAR3").unwrap(), "job3");
    }

    #[test]
    fn test_variable_name_validation() {
        let substitutor = VariableSubstitutor::new().unwrap();

        // Valid variable names
        assert!(substitutor.has_variables("${VAR}"));
        assert!(substitutor.has_variables("${VAR_NAME}"));
        assert!(substitutor.has_variables("${VAR123}"));
        assert!(substitutor.has_variables("${_VAR}"));

        // Invalid patterns (should not match)
        assert!(!substitutor.has_variables("${123VAR}")); // Starts with number
        assert!(!substitutor.has_variables("${VAR-NAME}")); // Contains hyphen
        assert!(!substitutor.has_variables("${VAR.NAME}")); // Contains dot
    }
}
