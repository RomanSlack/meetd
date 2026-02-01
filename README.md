# meetd 📅 — Let your AI schedule meetings with other AIs

<p align="center">
  <img src="readme_images/meetd_readme_banner.jpg" alt="meetd banner" width="700"/>
</p>

<p align="center">
  <em>Agent-to-agent meeting scheduling with cryptographic trust.<br/>
  Your Claude schedules coffee with their Claude.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/axum-000000?style=flat&logo=rust&logoColor=white" alt="axum"/>
  <img src="https://img.shields.io/badge/SQLite-003B57?style=flat&logo=sqlite&logoColor=white" alt="SQLite"/>
  <img src="https://img.shields.io/badge/Google_Calendar-4285F4?style=flat&logo=google-calendar&logoColor=white" alt="Google Calendar"/>
  <img src="https://img.shields.io/badge/Ed25519-Signing-blue?style=flat" alt="Ed25519"/>
</p>

---

## What is this?

A CLI + API that lets AI assistants (Claude Code, etc.) schedule meetings on your behalf by talking to other people's AI agents. Proposals are Ed25519 signed for trust. Built in Rust for cheap hosting.

## Install

```bash
# Option 1: From crates.io (recommended)
cargo install meetd

# Option 2: Pre-built binaries (from GitHub releases)
# Download from https://github.com/RomanSlack/meetd/releases

# Option 3: Build from source
git clone https://github.com/RomanSlack/meetd && cd meetd
cargo build --release
```

## Quick Start

```bash
# Login via CLI (opens browser for Google OAuth)
meetd login

# Or login via browser only (no CLI needed):
# Visit https://meetd.fly.dev/auth/google and copy your API key

# Find mutual availability
meetd avail --with alice@example.com --duration 30m --window "2026-02-01..2026-02-07" --json

# Send a proposal
meetd propose --to alice@example.com --slot "2026-02-03T10:00" --duration 30m --title "Coffee chat"

# Check inbox
meetd inbox --json

# Accept a proposal
meetd accept --proposal prop_xyz789
```

## Commands

| Command | Description |
|---------|-------------|
| `meetd login` | Authenticate with Google Calendar |
| `meetd config` | Set privacy level, webhook URL |
| `meetd avail` | Query mutual free time |
| `meetd propose` | Send a signed meeting proposal |
| `meetd accept` | Accept a proposal |
| `meetd decline` | Decline a proposal |
| `meetd inbox` | View pending proposals |
| `meetd serve` | Run the API server |

All commands support `--json` for machine-readable output.

## REST API

Agents can also use the API directly without installing the CLI:

```bash
# Check availability
curl -X POST https://meetd.fly.dev/v1/availability \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"with_email": "alice@example.com", "duration_minutes": 30, "window_start": "2026-02-01T00:00:00Z", "window_end": "2026-02-07T23:59:59Z"}'

# Create proposal
curl -X POST https://meetd.fly.dev/v1/proposals \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"to_email": "alice@example.com", "slot_start": "2026-02-03T10:00:00Z", "duration_minutes": 30, "title": "Coffee chat"}'

# List inbox
curl https://meetd.fly.dev/v1/inbox -H "Authorization: Bearer $API_KEY"

# Accept/decline
curl -X POST https://meetd.fly.dev/v1/proposals/prop_xyz/accept -H "Authorization: Bearer $API_KEY"
```

Get your API key by running `meetd login` once. See [skill.md](skill.md) for full API docs.

## Privacy Levels

```bash
meetd config visibility busy_only   # Only share free/busy (default)
meetd config visibility masked      # Share "Busy: Meeting"
meetd config visibility full        # Share event titles
```

## Webhooks

Get notified when proposals arrive:

```bash
meetd config webhook https://my-agent.example.com/inbox
```

Events: `proposal.received`, `proposal.accepted`, `proposal.declined`, `proposal.expired`

## Self-Hosting

```bash
# Run server
GOOGLE_CLIENT_ID=xxx GOOGLE_CLIENT_SECRET=xxx \
  meetd serve --port 8080 --db ./meetd.db

# Or with Docker
docker build -t meetd .
docker run -p 8080:8080 -v ./data:/home/meetd/data \
  -e GOOGLE_CLIENT_ID=xxx -e GOOGLE_CLIENT_SECRET=xxx meetd
```

## How It Works

1. **You**: `meetd propose --to bob@example.com --slot "2026-02-03T10:00"`
2. **Server**: Creates Ed25519 signed proposal, notifies Bob's agent via webhook
3. **Bob's Agent**: Reviews proposal, checks Bob's calendar, accepts/declines
4. **Both**: Calendar events created automatically

## License

Apache 2.0
