use anyhow::{Context, Result};
use serde::Serialize;
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::oneshot;

use super::OutputFormat;
use crate::models::LocalConfig;

/// Response from login command
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub email: String,
    pub api_key: String,
    pub user_id: String,
}

impl std::fmt::Display for LoginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Logged in as {}", self.email)?;
        writeln!(f, "Your API key: {}", self.api_key)?;
        writeln!(f, "User ID: {}", self.user_id)
    }
}


/// Run the login command using server-side OAuth
pub async fn run_login(server_url: &str, format: OutputFormat) -> Result<()> {
    // Find an available port for the local callback server
    let listener = TcpListener::bind("127.0.0.1:0")
        .context("Failed to bind local server")?;
    let port = listener.local_addr()?.port();
    let callback_url = format!("http://localhost:{}", port);

    // Build the OAuth URL
    let auth_url = format!(
        "{}/auth/google?cli_callback={}",
        server_url,
        urlencoding::encode(&callback_url)
    );

    println!("Opening browser for authentication...");
    println!("If it doesn't open automatically, visit: {}", auth_url);

    // Open browser
    if let Err(e) = open::that(&auth_url) {
        eprintln!("Failed to open browser: {}. Please open the URL manually.", e);
    }

    // Set up channel for receiving the result
    let (tx, rx) = oneshot::channel::<Result<LoginResponse>>();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    // Start the local HTTP server to receive the callback
    let listener = tokio::net::TcpListener::from_std(listener)?;

    println!("Waiting for authentication...");

    // Handle exactly one request
    let tx_clone = tx.clone();
    let handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            handle_callback(stream, tx_clone).await;
        }
    });

    // Wait for the callback with a timeout
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300), // 5 minute timeout
        rx,
    )
    .await
    .context("Authentication timed out")?
    .context("Failed to receive authentication result")??;

    // Cancel the listener task
    handle.abort();

    // Save config locally
    let mut config = LocalConfig::load().unwrap_or_default();
    config.api_key = Some(result.api_key.clone());
    config.server_url = Some(server_url.to_string());
    config.email = Some(result.email.clone());
    config.user_id = Some(result.user_id.clone());
    config.save().context("Failed to save config")?;

    format.print(&result);
    Ok(())
}

async fn handle_callback(
    mut stream: tokio::net::TcpStream,
    tx: Arc<std::sync::Mutex<Option<oneshot::Sender<Result<LoginResponse>>>>>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buffer = [0u8; 4096];
    let n = match stream.read(&mut buffer).await {
        Ok(n) => n,
        Err(_) => return,
    };

    let request = String::from_utf8_lossy(&buffer[..n]);

    // Parse the request to extract query parameters
    let result = parse_callback_request(&request);

    // Send response to browser
    let (status, body) = match &result {
        Ok(login) => (
            "200 OK",
            format!(
                r#"<!DOCTYPE html>
<html>
<head><title>meetd - Login Successful</title>
<style>
body {{ font-family: -apple-system, system-ui, sans-serif; text-align: center; padding: 50px; background: #f5f5f5; }}
.container {{ background: white; border-radius: 8px; padding: 40px; max-width: 400px; margin: 0 auto; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
h1 {{ color: #22c55e; }}
p {{ color: #666; }}
</style>
</head>
<body>
<div class="container">
<h1>Login Successful</h1>
<p>Welcome, {}!</p>
<p>You can close this window and return to the terminal.</p>
</div>
</body>
</html>"#,
                login.email
            ),
        ),
        Err(e) => (
            "400 Bad Request",
            format!(
                r#"<!DOCTYPE html>
<html>
<head><title>meetd - Login Failed</title>
<style>
body {{ font-family: -apple-system, system-ui, sans-serif; text-align: center; padding: 50px; background: #f5f5f5; }}
.container {{ background: white; border-radius: 8px; padding: 40px; max-width: 400px; margin: 0 auto; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
h1 {{ color: #ef4444; }}
p {{ color: #666; }}
</style>
</head>
<body>
<div class="container">
<h1>Login Failed</h1>
<p>{}</p>
<p>Please close this window and try again.</p>
</div>
</body>
</html>"#,
                e
            ),
        ),
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    );

    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;

    // Send result through channel
    if let Some(tx) = tx.lock().unwrap().take() {
        let _ = tx.send(result);
    }
}

fn parse_callback_request(request: &str) -> Result<LoginResponse> {
    // Extract the path from the HTTP request
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");

    // Parse query parameters
    let query = path.split('?').nth(1).unwrap_or("");
    let params: std::collections::HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    // Check for error
    if let Some(error) = params.get("error") {
        anyhow::bail!("{}", error);
    }

    // Extract credentials
    let api_key = params
        .get("api_key")
        .ok_or_else(|| anyhow::anyhow!("Missing api_key in callback"))?
        .clone();
    let email = params
        .get("email")
        .ok_or_else(|| anyhow::anyhow!("Missing email in callback"))?
        .clone();
    let user_id = params
        .get("user_id")
        .ok_or_else(|| anyhow::anyhow!("Missing user_id in callback"))?
        .clone();

    Ok(LoginResponse {
        email,
        api_key,
        user_id,
    })
}

/// Run the logout command
pub fn run_logout(format: OutputFormat) -> Result<()> {
    let config_path = LocalConfig::config_path();
    if config_path.exists() {
        std::fs::remove_file(&config_path).context("Failed to remove config file")?;
    }

    let response = super::SuccessResponse {
        message: "Logged out successfully".to_string(),
    };
    format.print(&response);
    Ok(())
}
