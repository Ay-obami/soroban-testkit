//! Regression test: rustdoc warnings fail CI in every job, not just
//! the one step that happens to set the flag.
//!
//! `BUILD_SPEC.md` §11 requires "cargo doc with no warnings" on every
//! PR, and rustdoc also emits warnings while collecting doc tests
//! (broken intra-doc links, invalid HTML, …). With `RUSTDOCFLAGS:
//! -D warnings` set only as step-level env on the `cargo doc` step,
//! every other rustdoc invocation — including the doc-test step that
//! compiles the README — let warnings scroll past a green run. The
//! flag now lives in the workflow-level `env` block, where it applies
//! to every job and every step.

mod common;

use common::read_repo_file;

#[test]
fn rustdoc_warnings_fail_ci_in_every_job() {
    let ci = read_repo_file(".github/workflows/ci.yml");

    // Everything above the first `jobs:` key is the workflow-level
    // scope: env declared there is inherited by every job and step.
    let workflow_header = ci
        .split("\njobs:")
        .next()
        .expect("ci.yml must contain a `jobs:` section");

    assert!(
        workflow_header.contains("RUSTDOCFLAGS: -D warnings"),
        "ci.yml must set `RUSTDOCFLAGS: -D warnings` in the workflow-level `env` \
         block (above `jobs:`). A step-level setting covers only that one step: \
         the doc-test step and any future rustdoc invocation would compile with \
         warnings allowed and CI would stay green while rustdoc warns."
    );
}
