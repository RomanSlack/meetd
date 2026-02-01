use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

use crate::models::SignedProposal;

/// Ed25519 keypair for signing proposals
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    /// Generate a new random keypair
    pub fn generate() -> Result<Self> {
        let signing_key = SigningKey::generate(&mut OsRng);
        Ok(Self { signing_key })
    }

    /// Create from a base64-encoded private key
    pub fn from_private_key_base64(private_key: &str) -> Result<Self> {
        let bytes = BASE64.decode(private_key).context("Invalid base64 private key")?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;
        let signing_key = SigningKey::from_bytes(&key_bytes);
        Ok(Self { signing_key })
    }

    /// Get the public key as base64
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.signing_key.verifying_key().to_bytes())
    }

    /// Get the private key as base64 (for storage)
    pub fn private_key_base64(&self) -> String {
        BASE64.encode(self.signing_key.to_bytes())
    }

    /// Sign a message and return the signature as base64
    pub fn sign(&self, message: &str) -> String {
        let signature = self.signing_key.sign(message.as_bytes());
        BASE64.encode(signature.to_bytes())
    }

    /// Sign a proposal and return the signature
    pub fn sign_proposal(&self, proposal: &mut SignedProposal) -> String {
        let payload = proposal.signing_payload();
        let signature = self.sign(&payload);
        proposal.signature = signature.clone();
        signature
    }
}

/// Verify a signature using a public key
pub struct PublicKey {
    verifying_key: VerifyingKey,
}

impl PublicKey {
    /// Create from a base64-encoded public key
    pub fn from_base64(public_key: &str) -> Result<Self> {
        let bytes = BASE64.decode(public_key).context("Invalid base64 public key")?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;
        let verifying_key =
            VerifyingKey::from_bytes(&key_bytes).context("Invalid Ed25519 public key")?;
        Ok(Self { verifying_key })
    }

    /// Verify a signature
    pub fn verify(&self, message: &str, signature_base64: &str) -> Result<bool> {
        let signature_bytes = BASE64.decode(signature_base64).context("Invalid base64 signature")?;
        let sig_bytes: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid signature length"))?;
        let signature = Signature::from_bytes(&sig_bytes);

        match self.verifying_key.verify(message.as_bytes(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Verify a signed proposal
    pub fn verify_proposal(&self, proposal: &SignedProposal) -> Result<bool> {
        let payload = proposal.signing_payload();
        self.verify(&payload, &proposal.signature)
    }
}

/// Generate a random API key
pub fn generate_api_key() -> String {
    let random_bytes: [u8; 24] = rand::random();
    format!(
        "mdk_{}",
        BASE64.encode(random_bytes).replace(['+', '/', '='], "")
    )
}

/// Generate a random webhook secret
pub fn generate_webhook_secret() -> String {
    let random_bytes: [u8; 32] = rand::random();
    hex::encode(random_bytes)
}

/// Hash an API key for storage
pub fn hash_api_key(api_key: &str) -> Result<String> {
    bcrypt::hash(api_key, 10).context("Failed to hash API key")
}

/// Verify an API key against a hash
pub fn verify_api_key(api_key: &str, hash: &str) -> bool {
    bcrypt::verify(api_key, hash).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ProposalSlot, SignedProposal};
    use chrono::Utc;

    #[test]
    fn test_keypair_generation() {
        let keypair = Keypair::generate().unwrap();
        let pub_key = keypair.public_key_base64();
        let priv_key = keypair.private_key_base64();

        assert!(!pub_key.is_empty());
        assert!(!priv_key.is_empty());

        // Can recreate from private key
        let restored = Keypair::from_private_key_base64(&priv_key).unwrap();
        assert_eq!(restored.public_key_base64(), pub_key);
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = Keypair::generate().unwrap();
        let message = "Hello, World!";

        let signature = keypair.sign(message);

        let pub_key = PublicKey::from_base64(&keypair.public_key_base64()).unwrap();
        assert!(pub_key.verify(message, &signature).unwrap());

        // Wrong message should fail
        assert!(!pub_key.verify("Wrong message", &signature).unwrap());
    }

    #[test]
    fn test_sign_proposal() {
        let keypair = Keypair::generate().unwrap();

        let mut proposal = SignedProposal {
            version: 1,
            from: "alice@example.com".to_string(),
            from_pubkey: keypair.public_key_base64(),
            to: "bob@example.com".to_string(),
            slot: ProposalSlot {
                start: Utc::now(),
                duration_minutes: 30,
            },
            title: Some("Coffee chat".to_string()),
            description: None,
            nonce: uuid::Uuid::new_v4().to_string(),
            expires_at: Utc::now() + chrono::Duration::days(1),
            signature: String::new(),
        };

        keypair.sign_proposal(&mut proposal);

        let pub_key = PublicKey::from_base64(&proposal.from_pubkey).unwrap();
        assert!(pub_key.verify_proposal(&proposal).unwrap());
    }

    #[test]
    fn test_api_key_generation() {
        let key = generate_api_key();
        assert!(key.starts_with("mdk_"));
        assert!(key.len() > 10);
    }

    #[test]
    fn test_api_key_hashing() {
        let key = generate_api_key();
        let hash = hash_api_key(&key).unwrap();

        assert!(verify_api_key(&key, &hash));
        assert!(!verify_api_key("wrong_key", &hash));
    }
}
