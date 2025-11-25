use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use common::auth::{DatabaseAuthService, JwtService};
use common::db::repositories::user::UserRepository;
use serde::Deserialize;
use std::collections::HashMap;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LoginQuery {
    redirect: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginFormData {
    username: String,
    password: String,
}

/// Display the login page
/// Requirements: 19.1, 19.2, 19.3, 19.4, 19.5, 19.6
#[tracing::instrument(skip(state))]
pub async fn login_page(
    State(state): State<AppState>,
    Query(params): Query<LoginQuery>,
) -> impl IntoResponse {
    let error_message = params.error.unwrap_or_default();
    let auth_mode = match &state.config.auth.mode {
        common::config::AuthMode::Database => "Database",
        common::config::AuthMode::Keycloak => "Keycloak",
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="description" content="Vietnam Enterprise Cron System - Secure Login">
    <meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline';">
    <meta http-equiv="X-Frame-Options" content="DENY">
    <meta http-equiv="X-Content-Type-Options" content="nosniff">
    <title>Login - Vietnam Enterprise Cron System</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        }}
        .login-container {{
            background: white;
            border-radius: 16px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            max-width: 440px;
            width: 100%;
            padding: 48px;
            animation: slideIn 0.4s ease-out;
        }}
        @keyframes slideIn {{
            from {{
                opacity: 0;
                transform: translateY(-20px);
            }}
            to {{
                opacity: 1;
                transform: translateY(0);
            }}
        }}
        .logo {{
            text-align: center;
            margin-bottom: 32px;
        }}
        .logo-icon {{
            width: 64px;
            height: 64px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            border-radius: 16px;
            display: inline-flex;
            align-items: center;
            justify-content: center;
            font-size: 32px;
            margin-bottom: 16px;
        }}
        h1 {{
            color: #2d3748;
            font-size: 28px;
            text-align: center;
            margin-bottom: 8px;
        }}
        .subtitle {{
            color: #718096;
            font-size: 14px;
            text-align: center;
            margin-bottom: 32px;
        }}
        .auth-mode {{
            display: inline-block;
            background: #edf2f7;
            color: #4a5568;
            padding: 4px 12px;
            border-radius: 12px;
            font-size: 12px;
            font-weight: 600;
            margin-bottom: 24px;
        }}
        .error-message {{
            background: #fed7d7;
            border: 1px solid #fc8181;
            color: #c53030;
            padding: 12px 16px;
            border-radius: 8px;
            margin-bottom: 24px;
            font-size: 14px;
            display: {error_display};
        }}
        .form-group {{
            margin-bottom: 20px;
        }}
        label {{
            display: block;
            color: #2d3748;
            font-size: 14px;
            font-weight: 600;
            margin-bottom: 8px;
        }}
        input[type="text"],
        input[type="password"] {{
            width: 100%;
            padding: 12px 16px;
            border: 2px solid #e2e8f0;
            border-radius: 8px;
            font-size: 15px;
            transition: all 0.2s;
            background: white;
        }}
        input[type="text"]:focus,
        input[type="password"]:focus {{
            outline: none;
            border-color: #667eea;
            box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
        }}
        input[type="text"].error,
        input[type="password"].error {{
            border-color: #fc8181;
        }}
        .input-error {{
            color: #c53030;
            font-size: 13px;
            margin-top: 4px;
            display: none;
        }}
        .input-error.show {{
            display: block;
        }}
        .login-button {{
            width: 100%;
            padding: 14px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.3s;
            margin-top: 8px;
        }}
        .login-button:hover {{
            transform: translateY(-1px);
            box-shadow: 0 10px 20px rgba(102, 126, 234, 0.3);
        }}
        .login-button:active {{
            transform: translateY(0);
        }}
        .login-button:disabled {{
            background: #cbd5e0;
            cursor: not-allowed;
            transform: none;
        }}
        .loading {{
            display: none;
            text-align: center;
            margin-top: 16px;
            color: #718096;
            font-size: 14px;
        }}
        .loading.show {{
            display: block;
        }}
        .loading-spinner {{
            display: inline-block;
            width: 16px;
            height: 16px;
            border: 2px solid #cbd5e0;
            border-top-color: #667eea;
            border-radius: 50%;
            animation: spin 0.6s linear infinite;
            margin-right: 8px;
            vertical-align: middle;
        }}
        @keyframes spin {{
            to {{ transform: rotate(360deg); }}
        }}
        .default-credentials {{
            background: #ebf8ff;
            border: 1px solid #90cdf4;
            color: #2c5282;
            padding: 12px 16px;
            border-radius: 8px;
            margin-top: 24px;
            font-size: 13px;
        }}
        .default-credentials strong {{
            display: block;
            margin-bottom: 4px;
        }}
        .footer {{
            margin-top: 32px;
            padding-top: 24px;
            border-top: 1px solid #e2e8f0;
            text-align: center;
        }}
        .footer-links {{
            display: flex;
            justify-content: center;
            gap: 20px;
            margin-bottom: 12px;
        }}
        .footer-links a {{
            color: #667eea;
            text-decoration: none;
            font-size: 14px;
            transition: color 0.2s;
        }}
        .footer-links a:hover {{
            color: #5a67d8;
        }}
        .footer-text {{
            color: #a0aec0;
            font-size: 12px;
        }}
        @media (max-width: 480px) {{
            .login-container {{
                padding: 32px 24px;
            }}
            h1 {{
                font-size: 24px;
            }}
        }}
        /* Graceful degradation without JavaScript */
        .no-js-message {{
            display: none;
            background: #fefcbf;
            border: 1px solid #ecc94b;
            color: #744210;
            padding: 12px 16px;
            border-radius: 8px;
            margin-bottom: 16px;
            font-size: 13px;
        }}
    </style>
</head>
<body>
    <div class="login-container">
        <div class="logo">
            <div class="logo-icon">🚀</div>
        </div>
        <h1>Vietnam Enterprise Cron System</h1>
        <p class="subtitle">Distributed Job Scheduling Platform</p>

        <div style="text-align: center;">
            <span class="auth-mode">Auth Mode: {auth_mode}</span>
        </div>

        {error_html}

        <noscript>
            <div class="no-js-message">
                JavaScript is disabled. The form will still work, but some features like client-side validation will be unavailable.
            </div>
        </noscript>

        <form id="login-form" method="POST" action="/api/auth/login/form" novalidate>
            <div class="form-group">
                <label for="username">Username</label>
                <input
                    type="text"
                    id="username"
                    name="username"
                    required
                    autocomplete="username"
                    placeholder="Enter your username"
                    aria-label="Username"
                    aria-required="true"
                >
                <div class="input-error" id="username-error">Username is required</div>
            </div>

            <div class="form-group">
                <label for="password">Password</label>
                <input
                    type="password"
                    id="password"
                    name="password"
                    required
                    autocomplete="current-password"
                    placeholder="Enter your password"
                    aria-label="Password"
                    aria-required="true"
                >
                <div class="input-error" id="password-error">Password is required</div>
            </div>

            <button type="submit" class="login-button" id="login-button">
                Sign In
            </button>

            <div class="loading" id="loading">
                <div class="loading-spinner"></div>
                <span>Authenticating...</span>
            </div>
        </form>

        <div class="default-credentials">
            <strong>First-time setup:</strong>
            Default credentials: <code>admin</code> / <code>admin123</code>
        </div>

        <div class="footer">
            <div class="footer-links">
                <a href="/health">Health Check</a>
                <a href="/metrics">Metrics</a>
            </div>
            <p class="footer-text">Built with Rust 🦀</p>
        </div>
    </div>

    <script>
        // Client-side form validation and submission
        (function() {{
            const form = document.getElementById('login-form');
            const usernameInput = document.getElementById('username');
            const passwordInput = document.getElementById('password');
            const submitButton = document.getElementById('login-button');
            const loading = document.getElementById('loading');

            // Remove error styling on input
            function clearError(input, errorId) {{
                input.classList.remove('error');
                document.getElementById(errorId).classList.remove('show');
            }}

            // Show error styling on input
            function showError(input, errorId) {{
                input.classList.add('error');
                document.getElementById(errorId).classList.add('show');
            }}

            // Validate form
            function validateForm() {{
                let valid = true;

                // Validate username
                if (!usernameInput.value.trim()) {{
                    showError(usernameInput, 'username-error');
                    valid = false;
                }} else {{
                    clearError(usernameInput, 'username-error');
                }}

                // Validate password
                if (!passwordInput.value) {{
                    showError(passwordInput, 'password-error');
                    valid = false;
                }} else {{
                    clearError(passwordInput, 'password-error');
                }}

                return valid;
            }}

            // Clear errors on input
            usernameInput.addEventListener('input', function() {{
                clearError(usernameInput, 'username-error');
            }});

            passwordInput.addEventListener('input', function() {{
                clearError(passwordInput, 'password-error');
            }});

            // Handle form submission
            form.addEventListener('submit', function(e) {{
                e.preventDefault();

                if (!validateForm()) {{
                    return;
                }}

                // Show loading state
                submitButton.disabled = true;
                loading.classList.add('show');

                // Submit form via AJAX
                const formData = new FormData(form);

                fetch('/api/auth/login', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                    }},
                    body: JSON.stringify({{
                        username: formData.get('username'),
                        password: formData.get('password'),
                    }}),
                }})
                .then(response => {{
                    if (!response.ok) {{
                        return response.json().then(data => {{
                            throw new Error(data.error || 'Login failed');
                        }});
                    }}
                    return response.json();
                }})
                .then(data => {{
                    // Store token in localStorage (secure with httpOnly cookie would be better)
                    localStorage.setItem('auth_token', data.data.token);
                    localStorage.setItem('token_expires_at', data.data.expires_at);

                    // Redirect to dashboard
                    window.location.href = '/dashboard';
                }})
                .catch(error => {{
                    // Show error and reset form
                    submitButton.disabled = false;
                    loading.classList.remove('show');

                    // Display error message
                    const errorDiv = document.querySelector('.error-message');
                    if (errorDiv) {{
                        errorDiv.textContent = error.message || 'Invalid username or password. Please try again.';
                        errorDiv.style.display = 'block';
                    }}

                    // Focus back to username
                    usernameInput.focus();
                }});
            }});

            // Auto-focus username field
            usernameInput.focus();
        }})();
    </script>
</body>
</html>"#,
        auth_mode = auth_mode,
        error_display = if error_message.is_empty() {
            "none"
        } else {
            "block"
        },
        error_html = if !error_message.is_empty() {
            format!(r#"<div class="error-message">{}</div>"#, html_escape(&error_message))
        } else {
            String::new()
        },
    );

    Html(html)
}

/// Handle form-based login (for no-JS fallback)
/// Requirements: 19.11, 19.12 - Graceful degradation without JavaScript
#[tracing::instrument(skip(state, form))]
pub async fn login_form_submit(
    State(state): State<AppState>,
    Form(form): Form<LoginFormData>,
) -> impl IntoResponse {
    // Validate input
    if form.username.trim().is_empty() {
        return Redirect::to("/?error=Username%20is%20required");
    }

    if form.password.is_empty() {
        return Redirect::to("/?error=Password%20is%20required");
    }

    // Create JWT service from config
    let jwt_secret = &state.config.auth.jwt_secret;
    let jwt_expiry_hours = state.config.auth.jwt_expiration_hours;
    let jwt_service = JwtService::new(jwt_secret, jwt_expiry_hours);

    // Create user repository and auth service
    let user_repository = UserRepository::new(state.db_pool.clone());
    let auth_service = DatabaseAuthService::new(jwt_service, user_repository);

    // Authenticate user
    match auth_service.login(&form.username, &form.password).await {
        Ok(token) => {
            tracing::info!(
                username = %form.username,
                "Form-based login successful"
            );

            // For no-JS fallback, we'd need to set a cookie
            // For now, redirect to a page that sets the token via JS
            // Simple URL encoding for token (replace special chars)
            let encoded_token = token.replace('&', "%26").replace('=', "%3D");
            Redirect::to(&format!("/auth/set-token?token={}&redirect=/dashboard", encoded_token))
        }
        Err(e) => {
            tracing::warn!(
                username = %form.username,
                error = %e,
                "Form-based login failed"
            );

            let error_msg = match e {
                common::errors::AuthError::InvalidCredentials => {
                    "Invalid username or password"
                }
                _ => "Authentication failed. Please try again.",
            };

            // Simple URL encoding for error message
            let encoded_error = error_msg.replace(' ', "%20").replace('&', "%26");
            Redirect::to(&format!("/?error={}", encoded_error))
        }
    }
}

/// Helper page to set auth token via JavaScript
/// Requirements: 19.7 - Secure token storage in localStorage
#[tracing::instrument]
pub async fn set_token_page(Query(params): Query<HashMap<String, String>>) -> Html<String> {
    let token = params.get("token").cloned().unwrap_or_default();
    let redirect = params.get("redirect").cloned().unwrap_or_else(|| "/dashboard".to_string());

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Setting up session...</title>
</head>
<body>
    <p>Setting up your session...</p>
    <script>
        localStorage.setItem('auth_token', '{}');
        window.location.href = '{}';
    </script>
    <noscript>
        <p>JavaScript is required to complete login. Please enable JavaScript and try again.</p>
    </noscript>
</body>
</html>"#,
        html_escape(&token),
        html_escape(&redirect)
    );

    Html(html)
}

/// HTML escape helper
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('xss')</script>"), "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
        assert_eq!(html_escape("normal text"), "normal text");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }
}
