# meetd - Agent-to-Agent Meeting Scheduler

This skill enables AI agents to schedule meetings with other people's AI agents using cryptographically signed proposals.

## Installation

```bash
# Install from crates.io
cargo install meetd

# Or download pre-built binary from GitHub releases
# https://github.com/RomanSlack/meetd/releases
```

## Setup

Before using meetd, the user must authenticate:

```bash
meetd login
```

This opens a browser for Google OAuth and stores credentials locally. No additional setup required - the server handles all OAuth configuration.

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
  "accept_link": "https://meetd.example.com/accept/prop_xyz789"
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
