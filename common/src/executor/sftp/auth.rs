// SFTP authentication utilities
// Requirements: 19.3, 19.4 - Authentication methods
// RECC 2025: Small utility file

use crate::errors::ExecutionError;
use crate::models::SftpAuth;
use ssh2::Session;

/// Authenticate SSH session (re-exported from connection module)
pub fn authenticate_session(sess: &Session, auth: &SftpAuth) -> Result<(), ExecutionError> {
    // Implementation is in connection.rs
    // This is just a public API wrapper
    match auth {
        SftpAuth::Password { username, password } => {
            sess.userauth_password(username, password).map_err(|e| {
                ExecutionError::SftpAuthenticationFailed(format!(
                    "Password authentication failed: {}",
                    e
                ))
            })
        }
        SftpAuth::SshKey {
            username,
            private_key_path,
        } => sess
            .userauth_pubkey_file(username, None, std::path::Path::new(private_key_path), None)
            .map_err(|e| {
                ExecutionError::SftpAuthenticationFailed(format!(
                    "SSH key authentication failed: {}",
                    e
                ))
            }),
    }
}
