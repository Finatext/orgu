use std::process::ExitCode;

use anyhow::Result;

use orgu::cli::run;

#[allow(clippy::use_debug)]
#[tokio::main]
async fn main() -> Result<ExitCode> {
    run().await
}
