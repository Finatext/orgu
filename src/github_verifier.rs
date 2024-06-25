use anyhow::{bail, Context as _, Result};
use hex::encode;
use hmac::{Hmac, Mac};
use http::HeaderMap;
use sha2::Sha256;
use subtle::ConstantTimeEq;

pub trait GithubRequestVerifier {
    fn verify_request(headers: &HeaderMap, body: &str, secret: &str) -> Result<()>;
}

pub struct DefaultVerifier;

impl GithubRequestVerifier for DefaultVerifier {
    fn verify_request(headers: &HeaderMap, body: &str, secret: &str) -> Result<()> {
        let signature = headers
            .get("x-hub-signature-256")
            .with_context(|| "missing x-hub-signature-256 header field")?;

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .with_context(|| "HMAC creation failed")?;
        mac.update(body.as_bytes());
        let computed = encode(mac.finalize().into_bytes());
        let formatted = format!("sha256={computed}");
        // Into bool will be false if it's ok, so we need to negate it.
        let choice = !formatted.as_bytes().ct_eq(signature.as_bytes());
        if choice.into() {
            bail!(
                "comparison failed: signature={}, computed={}",
                signature.to_str()?,
                formatted,
            )
        }
        Ok(())
    }
}

// mockall for static methods needs synchronization, so availd it.
#[cfg(test)]
pub mod test {
    use super::*;

    pub struct NullVerifier;

    impl GithubRequestVerifier for NullVerifier {
        fn verify_request(_headers: &HeaderMap, _body: &str, _secret: &str) -> Result<()> {
            Ok(())
        }
    }

    pub struct FailVerifier;

    impl GithubRequestVerifier for FailVerifier {
        fn verify_request(_headers: &HeaderMap, _body: &str, _secret: &str) -> Result<()> {
            bail!("always failed")
        }
    }
}
