# meetd - Agent-to-Agent Meeting Scheduler

This skill enables AI agents to schedule meetings with other people's AI agents using cryptographically signed proposals.

## Installation (for agents)

If `meetd` is not installed, install it first:

```bash
# Option 1: Install via cargo (if Rust is available)
cargo install meetd

# Option 2: Download pre-built binary
# Linux x86_64:
curl -L https://github.com/RomanSlack/meetd/releases/latest/download/meetd-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv meetd /usr/local/bin/

# macOS (Apple Silicon):
curl -L https://github.com/RomanSlack/meetd/releases/latest/download/meetd-aarch64-apple-darwin.tar.gz | tar xz
sudo mv meetd /usr/local/bin/

# macOS (Intel):
curl -L https://github.com/RomanSlack/meetd/releases/latest/download/meetd-x86_64-apple-darwin.tar.gz | tar xz
sudo mv meetd /usr/local/bin/
```

## Setup

Before using meetd, the user must authenticate once to get an API key:

**Option A: Via CLI**
```bash
meetd login
```

**Option B: Via Browser (no CLI needed)**
1. Visit https://meetd.fly.dev/auth/google
2. Login with Google
3. Copy your API key from the page
4. Save it: `mkdir -p ~/.config/meetd && echo '{"api_key":"YOUR_KEY","server_url":"https://meetd.fly.dev"}' > ~/.config/meetd/config.json`

No Google Cloud setup required - the server handles OAuth.

## Commands

All commands support `--json` for machine-readable output.

### Check Availability

Find mutual free time with another person:

```bash
meetd avail --with alice@example.com --duration 30m --window "2026-02-01..2026-02-07" --json
```

Response:
```json
{
  "slots": [
    {"start": "2026-02-03T10:00:00Z", "end": "2026-02-03T10:30:00Z", "score": 0.9},
    {"start": "2026-02-03T14:00:00Z", "end": "2026-02-03T14:30:00Z", "score": 0.8}
  ]
}
```

Slots are scored by preference (working hours, not too soon, etc).

### Create Proposal

Send a meeting proposal to another person:

```bash
meetd propose --to alice@example.com --slot "2026-02-03T10:00" --duration 30m --title "Coffee chat" --json
```

Response:
```json
{
  "proposal_id": "prop_xyz789",
  "signed_proposal": "eyJ0eXAiOiJ...",
  "accept_link": "https://meetd.fly.dev/accept/prop_xyz789"
}
```

The `signed_proposal` is a cryptographically signed payload that can be sent to another agent.

### Check Inbox

View pending proposals:

```bash
meetd inbox --json
```

Response:
```json
{
  "proposals": [
    {
      "id": "prop_abc123",
      "from": "alice@example.com",
      "slot": {"start": "2026-02-03T10:00:00Z", "duration_minutes": 30},
      "title": "Quick sync",
      "expires_at": "2026-02-02T23:59:59Z"
    }
  ]
}
```

### Accept/Decline Proposals

```bash
meetd accept --proposal prop_abc123 --json
meetd decline --proposal prop_abc123 --json
```

### Accept Signed Proposal (Agent-to-Agent)

When receiving a signed proposal from another agent:

```bash
meetd accept-signed --signed "eyJ0eXAiOiJ..." --json
```

## Agent Workflow

### Scheduling a Meeting

1. **Check availability** with the target person
2. **Create a proposal** for the best slot
3. **Wait for response** (check inbox or receive webhook)

### Receiving a Meeting Request

1. **Check inbox** for pending proposals
2. **Review** the proposal details
3. **Accept or decline** based on user preferences

## Webhook Notifications

Agents can register a webhook to receive real-time notifications:

```bash
meetd config webhook https://my-agent.example.com/inbox
```

Webhook events:
- `proposal.received` - New meeting proposal
- `proposal.accepted` - Your proposal was accepted
- `proposal.declined` - Your proposal was declined
- `proposal.expired` - Proposal expired

Webhook payloads include HMAC signature in `X-Meetd-Signature` header for verification.

## Privacy Levels

Users control how much calendar info is shared:

- `busy_only` - Only share busy/free status (default)
- `masked` - Share "Busy: Meeting" without details
- `full` - Share event titles (never attendees/description)

Set with:
```bash
meetd config visibility busy_only
```

## Signed Proposal Format

Proposals are Ed25519 signed for authenticity:

```json
{
  "version": 1,
  "from": "roman@example.com",
  "from_pubkey": "base64...",
  "to": "alice@example.com",
  "slot": {
    "start": "2026-02-03T10:00:00Z",
    "duration_minutes": 30
  },
  "title": "Coffee chat",
  "nonce": "random-uuid",
  "expires_at": "2026-02-02T23:59:59Z",
  "signature": "base64-ed25519-sig"
}
```

## Tips for Agents

1. **Always check availability first** before proposing a time
2. **Include clear titles** so the recipient knows what the meeting is about
3. **Set reasonable expiration** - proposals expire after 7 days by default
4. **Handle declined proposals gracefully** - propose an alternative time
5. **Use webhooks** for real-time updates when possible
6. **Verify signatures** when receiving proposals from unknown agents

## Example: Full Scheduling Flow

```bash
# 1. Find available times
meetd avail --with bob@example.com --duration 30m --window "2026-02-01..2026-02-07" --json

# 2. Create proposal for best slot
meetd propose --to bob@example.com --slot "2026-02-03T10:00" --duration 30m --title "Project sync" --json

# 3. Check for response later
meetd sent --json

# Or on Bob's side:
# 4. Check inbox
meetd inbox --json

# 5. Accept the proposal
meetd accept --proposal prop_xyz789 --json
```

## REST API (Alternative to CLI)

Agents can call the API directly without installing the CLI. Base URL: `https://meetd.fly.dev`

### Authentication

All endpoints (except login) require Bearer token:
```
Authorization: Bearer mdk_xxxYourApiKeyxxx
```

To get an API key, the user must complete OAuth login via browser once.

### Endpoints

**Check Availability**
```bash
curl -X POST https://meetd.fly.dev/v1/availability \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "with_email": "alice@example.com",
    "duration_minutes": 30,
    "window_start": "2026-02-01T00:00:00Z",
    "window_end": "2026-02-07T23:59:59Z"
  }'
```

**Create Proposal**
```bash
curl -X POST https://meetd.fly.dev/v1/proposals \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "to_email": "alice@example.com",
    "slot_start": "2026-02-03T10:00:00Z",
    "duration_minutes": 30,
    "title": "Coffee chat"
  }'
```

**List Inbox**
```bash
curl https://meetd.fly.dev/v1/inbox \
  -H "Authorization: Bearer $API_KEY"
```

**Accept Proposal**
```bash
curl -X POST https://meetd.fly.dev/v1/proposals/prop_xyz789/accept \
  -H "Authorization: Bearer $API_KEY"
```

**Decline Proposal**
```bash
curl -X POST https://meetd.fly.dev/v1/proposals/prop_xyz789/decline \
  -H "Authorization: Bearer $API_KEY"
```

**Get Public Key (for signature verification)**
```bash
curl https://meetd.fly.dev/v1/agent/pubkey/alice@example.com
```

### Response Format

All responses are JSON. Errors include an `error` field:
```json
{"error": "Not authorized"}
```
