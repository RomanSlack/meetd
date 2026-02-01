-- Users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,           -- UUID
    email TEXT UNIQUE NOT NULL,
    google_refresh_token TEXT,     -- Encrypted
    public_key TEXT NOT NULL,      -- Ed25519 public key (base64)
    private_key TEXT NOT NULL,     -- Ed25519 private key (encrypted, base64)
    api_key_hash TEXT NOT NULL,    -- bcrypt hash
    visibility TEXT DEFAULT 'busy_only',  -- busy_only | masked | full
    webhook_url TEXT,              -- Optional: URL to POST notifications
    webhook_secret TEXT,           -- HMAC secret for webhook signature
    created_at INTEGER NOT NULL
);

-- Index for email lookups
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- Proposals table
CREATE TABLE IF NOT EXISTS proposals (
    id TEXT PRIMARY KEY,           -- UUID
    from_user_id TEXT NOT NULL,
    to_email TEXT NOT NULL,
    slot_start INTEGER NOT NULL,   -- Unix timestamp
    duration_minutes INTEGER NOT NULL,
    title TEXT,
    description TEXT,
    nonce TEXT NOT NULL,           -- Replay protection
    expires_at INTEGER NOT NULL,
    signature TEXT NOT NULL,       -- Ed25519 signature
    status TEXT DEFAULT 'pending', -- pending | accepted | declined | expired
    created_at INTEGER NOT NULL,
    FOREIGN KEY (from_user_id) REFERENCES users(id)
);

-- Index for recipient lookups
CREATE INDEX IF NOT EXISTS idx_proposals_to_email ON proposals(to_email);

-- Index for sender lookups
CREATE INDEX IF NOT EXISTS idx_proposals_from_user ON proposals(from_user_id);

-- Index for status filtering
CREATE INDEX IF NOT EXISTS idx_proposals_status ON proposals(status);

-- Nonce tracking for replay protection
CREATE TABLE IF NOT EXISTS used_nonces (
    nonce TEXT PRIMARY KEY,
    used_at INTEGER NOT NULL
);
