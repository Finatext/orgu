# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

orgu is a Rust-based tool for implementing organization-wide CI workflows on GitHub. It serves as a cost-effective alternative to GitHub Enterprise's ruleset workflows, allowing organizations on Team plans to run centralized CI jobs across multiple repositories.

The system consists of two main components:
- **orgu-front**: Receives GitHub webhook events, filters them, and forwards to an event queue
- **orgu-runner**: Processes events from the queue, executes CI jobs, and reports results via GitHub Checks API

## Architecture

The codebase follows a modular structure:
- `src/front/`: Frontend component handling webhooks and event forwarding
- `src/runner/`: Runner component executing jobs and managing repository checkouts
- `src/cli/`: CLI command definitions and routing
- `src/events.rs`: Core event data structures (CheckRequest, GithubRepository)
- `src/checkout.rs`: Repository checkout logic using libgit2
- `src/github_client.rs`: GitHub API interactions via octorust
- `src/github_token.rs`: JWT-based GitHub App token generation
- `example/`: Example Docker container setup for orgu-runner with gitleaks

## Key Patterns and Conventions

### Error Handling
- Use `anyhow::Result` for CLI commands and top-level error handling
- Use `thiserror` for domain-specific errors (see `CheckoutError`)
- Propagate errors with context: `.with_context(|| "descriptive message")?`

### Async Runtime
- Uses `tokio` runtime throughout
- Blocking operations (like git operations) wrapped in `spawn_blocking`
- Timeout handling for long-running operations (checkout, job execution)

### Trait-Based Design
Core functionality abstracted via traits for testability:
- `GithubClient`: GitHub API interactions (mockable with `MockGithubClient`)
- `Checkout`: Repository checkout logic (mockable with `MockCheckout`)
- `TokenFetcher`: GitHub token generation (mockable with `MockTokenFetcher`)

### Configuration
- All runtime config via environment variables
- CLI args defined with `clap` derive macros
- Use `#[clap(long, env)]` for dual CLI/env support
- Humantime durations for timeout values (e.g., "10m", "30s")

## Testing Strategy
- Unit tests in same file as implementation (see `checkout.rs`, `runner/handler.rs`)
- Integration tests use `mockall` for mocking traits
- Use `axum-test` for HTTP endpoint testing
- Test fixtures in test modules (e.g., `build_checkrequest()`)

## Clippy Configuration

This project uses extensive clippy lints (see `Cargo.toml`). Notable restrictions:
- No `unwrap()` or `expect()` (use `?` operator)
- No `panic!()`, `todo!()`, `unimplemented!()`
- No indexing/slicing without explicit allow
- Nursery lints enabled

To allow specific lints in code:
```rust
#[allow(clippy::indexing_slicing)]
```

## Release Process

1. Update version in `Cargo.toml`
2. Regenerate `Cargo.lock`: `cargo build`
3. Create PR and merge
4. Create git tag: `git tag "$(cargo metadata --no-deps --format-version 1 | jq -r '"v" + .packages[0].version')"`
5. Push tag to trigger CI/CD
6. CI builds binaries and Docker images, creates draft GitHub Release
7. Update Homebrew formula: `GITHUB_REF="refs/tags/v<version>" TARGET=orgu .github/scripts/update_formula`

## Important Implementation Notes

- Runner jobs execute in checked-out repository directory (working dir set to repo)
- Checkout uses depth=1 by default for faster clones
- Job timeouts default to 10 minutes (configurable via `--job-timeout`)
- Check run updates handle both success and failure cases without failing orgu itself
- Re-run events filtered by installation ID to prevent cross-App triggering
