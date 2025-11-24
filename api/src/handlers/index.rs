use axum::response::Html;

/// Index/landing page handler
/// Returns a simple HTML page with API information and links
#[tracing::instrument]
pub async fn index() -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Vietnam Enterprise Cron System</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 20px;
        }
        .container {
            background: white;
            border-radius: 16px;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
            max-width: 800px;
            width: 100%;
            padding: 48px;
        }
        h1 {
            color: #2d3748;
            font-size: 32px;
            margin-bottom: 12px;
        }
        .subtitle {
            color: #718096;
            font-size: 18px;
            margin-bottom: 32px;
        }
        .status {
            display: inline-block;
            background: #48bb78;
            color: white;
            padding: 6px 16px;
            border-radius: 20px;
            font-size: 14px;
            font-weight: 600;
            margin-bottom: 32px;
        }
        .section {
            margin-bottom: 32px;
        }
        .section h2 {
            color: #2d3748;
            font-size: 20px;
            margin-bottom: 16px;
        }
        .endpoint-list {
            list-style: none;
        }
        .endpoint-list li {
            background: #f7fafc;
            padding: 12px 16px;
            margin-bottom: 8px;
            border-radius: 8px;
            border-left: 4px solid #667eea;
        }
        .endpoint-list code {
            color: #667eea;
            font-family: 'Courier New', monospace;
            font-size: 14px;
        }
        .endpoint-list .desc {
            color: #718096;
            font-size: 13px;
            margin-top: 4px;
        }
        .info-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 16px;
            margin-bottom: 32px;
        }
        .info-card {
            background: #edf2f7;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
        }
        .info-card .label {
            color: #718096;
            font-size: 13px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 8px;
        }
        .info-card .value {
            color: #2d3748;
            font-size: 24px;
            font-weight: 600;
        }
        .button {
            display: inline-block;
            background: #667eea;
            color: white;
            padding: 12px 24px;
            border-radius: 8px;
            text-decoration: none;
            font-weight: 600;
            transition: background 0.3s;
        }
        .button:hover {
            background: #5a67d8;
        }
        .footer {
            margin-top: 32px;
            padding-top: 24px;
            border-top: 1px solid #e2e8f0;
            color: #a0aec0;
            font-size: 14px;
            text-align: center;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 Vietnam Enterprise Cron System</h1>
        <p class="subtitle">Distributed Job Scheduling Platform</p>
        <span class="status">✓ System Online</span>

        <div class="info-grid">
            <div class="info-card">
                <div class="label">Version</div>
                <div class="value">1.0.0</div>
            </div>
            <div class="info-card">
                <div class="label">Architecture</div>
                <div class="value">Rust</div>
            </div>
            <div class="info-card">
                <div class="label">Status</div>
                <div class="value">Healthy</div>
            </div>
        </div>

        <div class="section">
            <h2>📡 API Endpoints</h2>
            <ul class="endpoint-list">
                <li>
                    <code>GET /health</code>
                    <div class="desc">Health check endpoint</div>
                </li>
                <li>
                    <code>POST /api/auth/login</code>
                    <div class="desc">Authenticate and get JWT token</div>
                </li>
                <li>
                    <code>GET /api/jobs</code>
                    <div class="desc">List all jobs (requires authentication)</div>
                </li>
                <li>
                    <code>GET /api/executions</code>
                    <div class="desc">View execution history (requires authentication)</div>
                </li>
                <li>
                    <code>GET /dashboard</code>
                    <div class="desc">Web dashboard (requires authentication)</div>
                </li>
                <li>
                    <code>GET /metrics</code>
                    <div class="desc">Prometheus metrics</div>
                </li>
            </ul>
        </div>

        <div class="section">
            <h2>🔐 Authentication</h2>
            <p style="color: #4a5568; margin-bottom: 16px;">
                This system uses JWT token authentication. To access protected endpoints:
            </p>
            <ol style="color: #4a5568; margin-left: 20px;">
                <li style="margin-bottom: 8px;">POST credentials to <code style="background: #edf2f7; padding: 2px 6px; border-radius: 4px;">/api/auth/login</code></li>
                <li style="margin-bottom: 8px;">Receive JWT token in response</li>
                <li style="margin-bottom: 8px;">Include token in Authorization header: <code style="background: #edf2f7; padding: 2px 6px; border-radius: 4px;">Bearer {token}</code></li>
            </ol>
            <p style="color: #718096; margin-top: 16px; font-size: 14px;">
                Default credentials: <strong>admin</strong> / <strong>admin123</strong>
            </p>
        </div>

        <div class="section">
            <h2>📚 Documentation</h2>
            <p style="color: #4a5568; margin-bottom: 16px;">
                For detailed API documentation, job definitions, and examples, please refer to the project README.
            </p>
        </div>

        <div class="footer">
            <p>Built with Rust 🦀 | Powered by Tokio, Axum, PostgreSQL, Redis, NATS, MinIO</p>
        </div>
    </div>
</body>
</html>"#;

    Html(html.to_string())
}
