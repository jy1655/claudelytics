# Contributing to Claudelytics

Thanks for contributing.

## Scope

This repository is a fork of `nwiizo/claudelytics` and focuses on:

- Keeping compatibility with current Claude Code outputs
- Maintaining accurate cost/pricing behavior
- Improving reliability and UX for CLI/TUI workflows

## Development Setup

1. Install Rust stable.
2. Clone this repository.
3. Run:

```bash
cargo build
cargo test
```

## Required Checks

Before opening a pull request, run:

```bash
cargo fmt --all
cargo test --quiet
cargo clippy --all-targets --all-features -- -D warnings
```

## Pull Request Guidelines

- Keep changes focused and atomic.
- Include tests for behavior changes when possible.
- Update docs for user-visible changes.
- For pricing/model updates, include source links in the PR description.

## Commit Style

Use concise imperative commit messages, for example:

- `feat: support new Claude model aliases`
- `fix: handle malformed transcript records`
- `docs: clarify pricing-cache behavior`

## Questions

If requirements are unclear, open an issue first and describe:

- current behavior
- expected behavior
- reproduction steps
