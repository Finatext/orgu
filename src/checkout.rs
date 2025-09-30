use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context as _, Result, bail};
use clap::Args;
use git2::{ErrorClass, ErrorCode, FetchOptions, Oid, Progress, Repository};
use tempfile::tempdir;
use thiserror::Error;
use tokio::{task::spawn_blocking, time::timeout};
use tracing::{Span, debug, info, info_span, instrument, trace, warn};

#[derive(Debug, Args, Clone)]
pub struct CheckoutConfig {
    /// Depth of the clone. Default is 1. Set 0 to clone the whole repository.
    #[arg(long, env, default_value = "1")]
    fetch_depth: i32,
    /// Don't fetch the repository and also don't checkout any commits. This is useful for partial fetching.
    #[arg(long, env, default_value = "false", conflicts_with = "fetch_depth")]
    no_fetch: bool,
    /// Timeout seconds for fetching the repository. Default is 10 mins.
    #[arg(long, env, default_value = "10mins")]
    fetch_timeout: humantime::Duration,
}

#[allow(clippy::indexing_slicing)]
#[cfg_attr(test, mockall::automock)]
pub trait Checkout: Sync + Send {
    /// Create new temporary directory and checkout given repository under the directory.
    async fn create_dir_and_checkout(&self, input: &CheckoutInput) -> Result<WorkDir>;
    /// Checkout given repository under given repository.
    async fn checkout_under(&self, input: &CheckoutInput, under: &Path) -> Result<()>;
}

#[derive(Error, Debug)]
pub enum CheckoutError {
    #[error("timeout fetching repository took too long: {0}")]
    Timeout(humantime::Duration),
}

#[derive(Debug, Clone)]
pub struct CheckoutInput {
    pub owner: String,
    pub repo: String,
    pub sha: String,
    pub token: String,
}

impl CheckoutInput {
    pub fn full_name(&self) -> String {
        [self.owner.to_owned(), self.repo.to_owned()].join("/")
    }
}

/// Checkout result. Holds the path to newly created temporary workding directory.
pub struct WorkDir {
    pub path: PathBuf,
    // To keep the temporary directory alive.
    pub _parent: tempfile::TempDir,
}

impl WorkDir {
    /// Explicitly cleanup the temporary directory.
    pub fn cleanup(self) -> Result<()> {
        self._parent.close().with_context(|| {
            format!(
                "failed to cleanup temporary directory: {}",
                self.path.display()
            )
        })
    }
}

#[derive(Debug)]
pub struct Libgit2Checkout {
    config: CheckoutConfig,
}

impl Libgit2Checkout {
    pub const fn new(config: CheckoutConfig) -> Self {
        Self { config }
    }
}

const REMOTE_NAME: &str = "origin";

impl Checkout for Libgit2Checkout {
    async fn create_dir_and_checkout(&self, input: &CheckoutInput) -> Result<WorkDir> {
        let temp = tempdir()?;
        let work_dir = temp.path().join(&input.repo);
        self.checkout_under(input, &work_dir).await?;
        Ok(WorkDir {
            path: work_dir,
            _parent: temp,
        })
    }

    #[instrument(
        skip(self, input),
        fields(
            owner = input.owner.as_str(),
            repo = input.repo.as_str(),
            sha = input.sha.as_str(),
            under = %under.display(),
        )
    )]
    async fn checkout_under(&self, input: &CheckoutInput, under: &Path) -> Result<()> {
        // If no_fetch is enabled, skip fetching and just set remote in fetch_with_timeout().
        let repo =
            fetch_with_timeout(under.to_path_buf(), input.clone(), self.config.clone()).await?;

        if self.config.no_fetch {
            info!("no_fetch is enabled, skipping checkout");
            return Ok(());
        }

        debug!("checking out commit: {}", input.sha);
        // checkout the specific commit.
        let oid = Oid::from_str(&input.sha).with_context(|| {
            format!(
                "failed to create Git Object ID, invalid commit SHA?: sha={}",
                input.sha
            )
        })?;
        let commit = repo.find_commit(oid)?;
        repo.checkout_tree(commit.as_object(), None)
            .with_context(|| format!("failed to checkout {}:{}", input.full_name(), input.sha))?;
        repo.set_head_detached(commit.id())?;

        Ok(())
    }
}

// Requires owned arguments to pass to another thread.
async fn fetch_with_timeout(
    under: PathBuf,
    input: CheckoutInput,
    config: CheckoutConfig,
) -> Result<Repository> {
    info!("fetching repository with timeout: {}", config.fetch_timeout);
    let should_cancel = Arc::new(AtomicBool::new(false));

    let sc = Arc::clone(&should_cancel);
    let c = config.clone();
    // To pass span which refers parents to another thread, explicitly create a new span and pass it.
    let span = info_span!("fetch");
    let task = spawn_blocking(move || fetch(span, sc, under, input, c));

    match timeout(config.fetch_timeout.into(), task).await {
        Ok(res) => res.with_context(|| "Failed to spwan blocking task")?,
        Err(_) => {
            should_cancel.store(true, Ordering::Relaxed);
            debug!(
                "fetching repository timed out, try to cancel the fetch: timeout={}",
                config.fetch_timeout
            );
            Err(CheckoutError::Timeout(config.fetch_timeout).into())
        }
    }
}

fn fetch(
    parent: Span,
    should_cancel: Arc<AtomicBool>,
    under: PathBuf,
    input: CheckoutInput,
    config: CheckoutConfig,
) -> Result<Repository> {
    let _guard = parent.enter();

    let repo = Repository::init(&under)
        .with_context(|| format!("failed init repository: {}", under.display()))?;

    let url = format!(
        "https://x-access-token:{}@github.com/{}",
        input.token,
        input.full_name(),
    );
    if let Err(e) = repo.remote(REMOTE_NAME, &url) {
        if e.class() == ErrorClass::Config && e.code() == ErrorCode::Exists {
            debug!("remote already exists: remote_name={REMOTE_NAME}");
        } else {
            bail!("failed add remote: remote_name={REMOTE_NAME}");
        }
    }

    if config.no_fetch {
        return Ok(repo);
    }

    let mut fetch_options = FetchOptions::new();
    fetch_options.depth(config.fetch_depth);
    let mut callbacks = git2::RemoteCallbacks::new();

    let cb = |progress: Progress| {
        if should_cancel.load(Ordering::Relaxed) {
            if let Ok(mut r) = repo.find_remote(REMOTE_NAME)
                && let Err(e) = r.stop()
            {
                warn!("failed to stop remote: {}", e);
            }
            false
        } else {
            show_remote_progress(progress)
        }
    };
    callbacks.transfer_progress(cb);
    fetch_options.remote_callbacks(callbacks);

    let mut remote = repo.find_remote(REMOTE_NAME)?;
    let refspec = &[input.sha];
    debug!("fetching refspec: {:?}", refspec);
    remote
        .fetch(refspec, Some(&mut fetch_options), None)
        .with_context(|| format!("failed to fetch repository: depth={}", config.fetch_depth))?;

    // Recreate Repository to avoid sharing between threads.
    let repo = Repository::init(&under)
        .with_context(|| format!("failed init repository: {}", under.display()))?;
    Ok(repo)
}

// libgit2 requires this signature.
#[allow(clippy::needless_pass_by_value)]
// https://github.com/libgit2/libgit2/blob/v1.8.0/examples/clone.c
fn show_remote_progress(progress: Progress) -> bool {
    if progress.received_objects() == 0 {
        debug!("objects to receive={}, ", progress.total_objects());
        return true;
    }

    if progress.received_objects() != progress.total_objects() {
        show_receiving_progress(&progress);
    } else {
        // reciving objects done.
        if progress.indexed_deltas() == 0 {
            debug!("deltas to resolve={}, ", progress.total_deltas());
            return true;
        }
        trace!(
            "Resolving deltas {}/{}",
            progress.indexed_deltas(),
            progress.total_deltas()
        );
    }
    true
}

#[allow(clippy::integer_division)] // precision is not important.
fn show_receiving_progress(progress: &Progress) {
    let network_percent = if progress.total_objects() > 0 {
        (100 * progress.received_objects()) / progress.total_objects()
    } else {
        0
    };
    let index_percent = if progress.total_objects() > 0 {
        (100 * progress.indexed_objects()) / progress.total_objects()
    } else {
        0
    };
    let kbytes = progress.received_bytes() / 1024;
    let total_objects = progress.total_objects();
    let received_objects = progress.received_objects();
    let indexed_objects = progress.indexed_objects();
    trace!(
        "net {network_percent}% ({kbytes} kb, {received_objects}/{total_objects})  /  idx {index_percent}% ({indexed_objects}/{total_objects})",
    );
}
