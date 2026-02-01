use anyhow::Result;
use clap::{Parser, Subcommand};
use std::net::SocketAddr;

use meetd::cli::{self, OutputFormat};
use meetd::DEFAULT_SERVER_URL;

#[derive(Parser)]
#[command(name = "meetd")]
#[command(about = "Agent-to-Agent Meeting Scheduler", version)]
struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Google Calendar
    Login {
        /// Server URL (default: https://meetd.example.com)
        #[arg(long, default_value = DEFAULT_SERVER_URL)]
        server: String,
    },
    /// Log out and remove local credentials
    Logout,
    /// Configure settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Query availability for a meeting
    Avail {
        /// Email of the person to meet with
        #[arg(long)]
        with: String,
        /// Duration (e.g., "30m", "1h")
        #[arg(long)]
        duration: String,
        /// Time window (e.g., "2026-02-01..2026-02-07")
        #[arg(long)]
        window: String,
        /// Timezone (e.g., "America/New_York")
        #[arg(long)]
        timezone: Option<String>,
    },
    /// Create a meeting proposal
    Propose {
        /// Email to send proposal to
        #[arg(long)]
        to: String,
        /// Proposed time slot (e.g., "2026-02-03T10:00")
        #[arg(long)]
        slot: String,
        /// Duration (e.g., "30m", "1h")
        #[arg(long)]
        duration: String,
        /// Meeting title
        #[arg(long)]
        title: Option<String>,
        /// Meeting description
        #[arg(long)]
        description: Option<String>,
    },
    /// Accept a proposal
    Accept {
        /// Proposal ID to accept
        #[arg(long)]
        proposal: String,
    },
    /// Decline a proposal
    Decline {
        /// Proposal ID to decline
        #[arg(long)]
        proposal: String,
    },
    /// View and manage inbox
    Inbox {
        /// Watch for new proposals (streaming)
        #[arg(long)]
        watch: bool,
        /// Filter by status (pending, accepted, declined, expired)
        #[arg(long)]
        status: Option<String>,
    },
    /// View sent proposals
    Sent,
    /// Accept a signed proposal (agent-to-agent)
    AcceptSigned {
        /// Base64-encoded signed proposal
        #[arg(long)]
        signed: String,
    },
    /// Run the API server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,
        /// Database file path
        #[arg(long, default_value = "./meetd.db")]
        db: String,
        /// Public server URL (for OAuth callbacks)
        #[arg(long, default_value = "http://localhost:8080")]
        url: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set visibility level
    Visibility {
        /// Visibility level: busy_only, masked, or full
        level: String,
    },
    /// Set webhook URL for notifications
    Webhook {
        /// Webhook URL (leave empty to remove)
        url: Option<String>,
    },
    /// Set server URL
    Server {
        /// Server URL
        url: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("meetd=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let format = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };

    match cli.command {
        Commands::Login { server } => {
            cli::run_login(&server, format).await?;
        }
        Commands::Logout => {
            cli::run_logout(format)?;
        }
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                cli::run_config_show(format).await?;
            }
            ConfigAction::Visibility { level } => {
                cli::run_config_visibility(&level, format).await?;
            }
            ConfigAction::Webhook { url } => {
                cli::run_config_webhook(url.as_deref(), format).await?;
            }
            ConfigAction::Server { url } => {
                cli::run_config_server(&url, format)?;
            }
        },
        Commands::Avail {
            with,
            duration,
            window,
            timezone,
        } => {
            cli::run_avail(&with, &duration, &window, timezone.as_deref(), format).await?;
        }
        Commands::Propose {
            to,
            slot,
            duration,
            title,
            description,
        } => {
            cli::run_propose(
                &to,
                &slot,
                &duration,
                title.as_deref(),
                description.as_deref(),
                format,
            )
            .await?;
        }
        Commands::Accept { proposal } => {
            cli::run_accept(&proposal, format).await?;
        }
        Commands::Decline { proposal } => {
            cli::run_decline(&proposal, format).await?;
        }
        Commands::Inbox { watch, status } => {
            if watch {
                cli::run_inbox_watch(format).await?;
            } else {
                cli::run_inbox(status.as_deref(), format).await?;
            }
        }
        Commands::Sent => {
            cli::run_sent(format).await?;
        }
        Commands::AcceptSigned { signed } => {
            cli::run_accept_signed(&signed, format).await?;
        }
        Commands::Serve { port, db, url } => {
            let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
            meetd::server::run_server(addr, &db, &url).await?;
        }
    }

    Ok(())
}
