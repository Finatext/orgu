use std::collections::HashMap;
use std::env;

use anyhow::{Context as _, Result};
use aws_sdk_ssm::client::Client;
use tracing::{debug, trace};

type EnvKey = String;
type ParameterName = String;
type FullParameterName = String;
type ParameterValue = String;

// Should be called in main thread exclusively, because it reads/writes environment variables.
pub async fn with_replaced_env<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> T,
{
    let original: HashMap<EnvKey, FullParameterName> = env::vars()
        .filter(|(_, v)| v.starts_with("ssm://"))
        .collect();
    trace!("original env vars: {:?}", original);
    if original.is_empty() {
        return Ok(f());
    }

    let names: Vec<ParameterName> = original
        .values()
        .map(|v| v.trim_start_matches("ssm://").to_owned())
        .collect();
    let fetched_values = fetch(names).await?;

    for (k, v) in original.iter() {
        let trimmed = v.trim_start_matches("ssm://");
        let value: ParameterValue = fetched_values
            .get(trimmed)
            .map(ToOwned::to_owned)
            .with_context(|| format!("no value fetched for {trimmed}"))?;
        env::set_var(k, value);
    }

    let res = f();

    // Restore original env vars.
    for (k, v) in original.iter() {
        env::set_var(k, v);
    }
    Ok(res)
}

async fn fetch(names: Vec<ParameterName>) -> Result<HashMap<ParameterName, ParameterValue>> {
    debug!("fetching SSM values for names: {}", names.join(", "));

    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);
    let res = client
        .get_parameters()
        .set_names(Some(names))
        .with_decryption(true)
        .send()
        .await?
        .parameters
        .with_context(|| "no parameter fetched")?;
    Ok(res.into_iter().flat_map(|p| p.name.zip(p.value)).collect())
}
