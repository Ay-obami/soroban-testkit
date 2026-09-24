# Contributing

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for a map of the workspace — crate
and module boundaries, how they depend on each other, and where a new
capability should live — before your first PR.

## Scope

Read `BUILD_SPEC.md` §3 (Scope boundaries) before opening an issue or PR.
In scope: anything that helps test a Soroban contract in-process with
`soroban-sdk`'s test environment. Out of scope: mainnet/testnet forking,
deployment tooling, frontend/JS testing, a test runner, a full benchmarking
framework, anything requiring network access at test time.

## Workflow

- Trunk-based development: short-lived branches off `main`, squash merge,
  linear history.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/),
  enforced in CI.
- Every PR must pass: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test --workspace`, `cargo doc` with no warnings, `cargo audit`,
  `cargo deny check` (see [Supply-chain policy](#supply-chain-policy)).
  Red CI blocks merge with no maintainer exception.
- CI also runs a scheduled job weekly against whatever `soroban-sdk` version
  is currently latest on crates.io (independent of the pinned version in
  `Cargo.toml`), so a breaking upstream release is caught before it shows up
  in a contributor's PR. It opens an issue automatically if it fails; it
  never blocks a PR.
- Every PR that changes user-facing behavior adds an entry to
  `CHANGELOG.md` under `## [Unreleased]` (see ["How to add an
  entry"](CHANGELOG.md#how-to-add-an-entry)). A PR that removes or renames a
  public item must also follow the versioning policy — see
  [Versioning and releases](#versioning-and-releases).

## Versioning and releases

Both workspace crates are version-locked at `0.x` and follow
[`API_STABILITY.md`](API_STABILITY.md): a minor bump may break the API, a
patch bump may not. The supported `soroban-sdk` / Stellar protocol versions
per release are in [`COMPATIBILITY.md`](COMPATIBILITY.md), and the
release checklist for both crates is in
[`RELEASING.md`](RELEASING.md). The `docs` and `release` workflows in
`.github/workflows/` enforce these on every PR and on every `v*` tag.

## Supply-chain policy

Dependencies are checked with [`cargo-deny`](https://embarkstudios.github.io/cargo-deny/),
configured in `deny.toml`, and enforced in CI (`deny` job). It checks:

- **Licenses** — every dependency's license must be in the `allow` list
  (currently the OSI/FSF-approved licenses this project's Apache-2.0
  license is compatible with). A new dependency under a license outside
  that list needs a documented exception in `deny.toml`, not a widened
  `allow` list.
- **Advisories** — no known-yanked crate versions; RustSec advisories are
  checked in the `audit` job (`cargo audit`) and mirrored here.
- **Sources** — dependencies must come from crates.io; an unlisted registry
  or git dependency fails the check unless explicitly allow-listed.
- **Bans** — no wildcard (`*`) version requirements.

Run it locally before adding a dependency:

```sh
cargo install cargo-deny --locked
cargo deny check
```

## Code conventions

- No `unwrap()` or `expect()` in library code outside tests.
- Assertion helpers panic deliberately (that is their contract), but always
  with `TestkitError` context, never a bare `panic!("...")` message.
- Every public item needs a doc comment with a runnable `# Example` block.
  `#![deny(missing_docs)]` is enforced from Module 8 onward.
- Every assertion helper needs two tests: one where it passes, one where it
  correctly fails (`#[should_panic(expected = "...")]`, pinning the
  message).
- Failure messages are a first-class feature of this crate. An assertion
  helper whose failure message doesn't tell the user what went wrong and
  what to look at is not done.

## Local setup

```sh
rustup show               # installs the pinned toolchain from rust-toolchain.toml
make check                # fmt + clippy + test, same as CI
```

## Good first issues

Labeled `good-first-issue`. Module 7 (`ttl`) is not beginner-friendly and
is never labeled as such.
add validation coverage for the recurring contract
