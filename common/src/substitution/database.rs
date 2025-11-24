// Database job variable substitution
// Requirements: 2.11, 2.12 - Variable substitution in connection strings and parameterized queries

use crate::errors::SubstitutionError;
#[cfg(test)]
use crate::models::DatabaseType;
use crate::models::{JobType, QueryType};
use crate::substitution::VariableSubstitutor;
use std::collections::HashMap;
use tracing::instrument;

/// Substitute variables in database job configuration
///
/// # Arguments
/// * `job_type` - The database job type configuration
/// * `variables` - HashMap of variable names to values
/// * `substitutor` - The variable substitutor instance
///
/// # Requirements
/// - 2.11: Variable substitution in connection strings
/// - 2.12: Parameterized query substitution for SQL injection prevention
///
/// # Returns
/// A new JobType with all variables substituted
///
/// # Errors
/// Returns SubstitutionError if any referenced variables are undefined
///
/// # Security Note
/// For SQL queries, this function substitutes variables in the query string itself,
/// but the actual execution should use parameterized queries to prevent SQL injection.
/// The worker executor is responsible for converting the substituted query into
/// a parameterized query before execution.
#[instrument(skip(job_type, variables, substitutor))]
pub fn substitute_database_job(
    job_type: &JobType,
    variables: &HashMap<String, String>,
    substitutor: &VariableSubstitutor,
) -> Result<JobType, SubstitutionError> {
    match job_type {
        JobType::DatabaseQuery {
            database_type,
            connection_string,
            query,
            query_type,
        } => {
            // Substitute connection string
            let substituted_connection_string =
                substitutor.substitute(connection_string, variables)?;
            tracing::debug!(
                original_conn = connection_string,
                substituted_conn = %substituted_connection_string,
                "Substituted connection string"
            );

            // Substitute query
            // Note: The actual parameterization happens at execution time
            let substituted_query = substitutor.substitute(query, variables)?;
            tracing::debug!(
                original_query_len = query.len(),
                substituted_query_len = substituted_query.len(),
                "Substituted query"
            );

            // Substitute query type parameters if it's a stored procedure
            let substituted_query_type = match query_type {
                QueryType::StoredProcedure {
                    procedure_name,
                    parameters,
                } => {
                    let substituted_procedure_name =
                        substitutor.substitute(procedure_name, variables)?;
                    let mut substituted_parameters = Vec::new();
                    for param in parameters {
                        let substituted_param = substitutor.substitute(param, variables)?;
                        substituted_parameters.push(substituted_param);
                    }
                    tracing::debug!(
                        procedure = %substituted_procedure_name,
                        param_count = substituted_parameters.len(),
                        "Substituted stored procedure"
                    );
                    QueryType::StoredProcedure {
                        procedure_name: substituted_procedure_name,
                        parameters: substituted_parameters,
                    }
                }
                QueryType::RawSql => QueryType::RawSql,
            };

            Ok(JobType::DatabaseQuery {
                database_type: database_type.clone(),
                connection_string: substituted_connection_string,
                query: substituted_query,
                query_type: substituted_query_type,
            })
        }
        _ => {
            // Not a database job, return as-is
            Ok(job_type.clone())
        }
    }
}

/// Extract variable placeholders from a SQL query for parameterization
///
/// This function identifies all variable placeholders in a query so they can be
/// replaced with parameterized query placeholders (e.g., $1, $2, $3 for PostgreSQL)
/// at execution time to prevent SQL injection.
///
/// # Arguments
/// * `query` - The SQL query with variable placeholders
/// * `substitutor` - The variable substitutor instance
///
/// # Returns
/// A vector of variable names found in the query, in order of appearance
pub fn extract_query_variables(query: &str, substitutor: &VariableSubstitutor) -> Vec<String> {
    substitutor.extract_variables(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_variables() -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("DB_HOST".to_string(), "localhost".to_string());
        vars.insert("DB_PORT".to_string(), "5432".to_string());
        vars.insert("DB_NAME".to_string(), "mydb".to_string());
        vars.insert("DB_USER".to_string(), "admin".to_string());
        vars.insert("DB_PASSWORD".to_string(), "secret".to_string());
        vars.insert("USER_ID".to_string(), "12345".to_string());
        vars.insert("STATUS".to_string(), "active".to_string());
        vars
    }

    #[test]
    fn test_substitute_connection_string() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::PostgreSQL,
            connection_string:
                "postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
                    .to_string(),
            query: "SELECT * FROM users".to_string(),
            query_type: QueryType::RawSql,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::DatabaseQuery {
                connection_string, ..
            } => {
                assert_eq!(
                    connection_string,
                    "postgresql://admin:secret@localhost:5432/mydb"
                );
            }
            _ => panic!("Expected DatabaseQuery"),
        }
    }

    #[test]
    fn test_substitute_raw_sql_query() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::PostgreSQL,
            connection_string: "postgresql://localhost/mydb".to_string(),
            query: "SELECT * FROM users WHERE id = ${USER_ID} AND status = ${STATUS}".to_string(),
            query_type: QueryType::RawSql,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::DatabaseQuery { query, .. } => {
                assert_eq!(
                    query,
                    "SELECT * FROM users WHERE id = 12345 AND status = active"
                );
            }
            _ => panic!("Expected DatabaseQuery"),
        }
    }

    #[test]
    fn test_substitute_stored_procedure() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::PostgreSQL,
            connection_string: "postgresql://localhost/mydb".to_string(),
            query: "CALL get_user_data".to_string(),
            query_type: QueryType::StoredProcedure {
                procedure_name: "get_user_data".to_string(),
                parameters: vec!["${USER_ID}".to_string(), "${STATUS}".to_string()],
            },
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::DatabaseQuery { query_type, .. } => match query_type {
                QueryType::StoredProcedure {
                    procedure_name,
                    parameters,
                } => {
                    assert_eq!(procedure_name, "get_user_data");
                    assert_eq!(parameters.len(), 2);
                    assert_eq!(parameters[0], "12345");
                    assert_eq!(parameters[1], "active");
                }
                _ => panic!("Expected StoredProcedure"),
            },
            _ => panic!("Expected DatabaseQuery"),
        }
    }

    #[test]
    fn test_substitute_mysql_connection() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::MySQL,
            connection_string: "mysql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
                .to_string(),
            query: "SELECT * FROM users".to_string(),
            query_type: QueryType::RawSql,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::DatabaseQuery {
                database_type,
                connection_string,
                ..
            } => {
                assert!(matches!(database_type, DatabaseType::MySQL));
                assert_eq!(
                    connection_string,
                    "mysql://admin:secret@localhost:5432/mydb"
                );
            }
            _ => panic!("Expected DatabaseQuery"),
        }
    }

    #[test]
    fn test_substitute_oracle_connection() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::Oracle,
            connection_string: "${DB_USER}/${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
                .to_string(),
            query: "SELECT * FROM users".to_string(),
            query_type: QueryType::RawSql,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::DatabaseQuery {
                database_type,
                connection_string,
                ..
            } => {
                assert!(matches!(database_type, DatabaseType::Oracle));
                assert_eq!(connection_string, "admin/secret@localhost:5432/mydb");
            }
            _ => panic!("Expected DatabaseQuery"),
        }
    }

    #[test]
    fn test_substitute_undefined_variable_in_connection_string() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = HashMap::new();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::PostgreSQL,
            connection_string: "postgresql://${UNDEFINED_USER}@localhost/mydb".to_string(),
            query: "SELECT * FROM users".to_string(),
            query_type: QueryType::RawSql,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor);
        assert!(result.is_err());

        match result {
            Err(SubstitutionError::UndefinedVariable { variables, .. }) => {
                assert!(variables.contains(&"UNDEFINED_USER".to_string()));
            }
            _ => panic!("Expected UndefinedVariable error"),
        }
    }

    #[test]
    fn test_substitute_undefined_variable_in_query() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = HashMap::new();

        let job_type = JobType::DatabaseQuery {
            database_type: DatabaseType::PostgreSQL,
            connection_string: "postgresql://localhost/mydb".to_string(),
            query: "SELECT * FROM users WHERE id = ${UNDEFINED_ID}".to_string(),
            query_type: QueryType::RawSql,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor);
        assert!(result.is_err());

        match result {
            Err(SubstitutionError::UndefinedVariable { variables, .. }) => {
                assert!(variables.contains(&"UNDEFINED_ID".to_string()));
            }
            _ => panic!("Expected UndefinedVariable error"),
        }
    }

    #[test]
    fn test_extract_query_variables() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let query = "SELECT * FROM users WHERE id = ${USER_ID} AND status = ${STATUS}";

        let vars = extract_query_variables(query, &substitutor);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0], "USER_ID");
        assert_eq!(vars[1], "STATUS");
    }

    #[test]
    fn test_substitute_non_database_job() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::HttpRequest {
            method: crate::models::HttpMethod::Get,
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        };

        let result = substitute_database_job(&job_type, &variables, &substitutor).unwrap();

        // Should return the same job type unchanged
        match result {
            JobType::HttpRequest { .. } => {}
            _ => panic!("Expected HttpRequest"),
        }
    }
}
