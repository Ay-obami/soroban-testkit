//! Regression test: crates.io metadata links stay aligned with the
//! repository URL.
//!
//! `BUILD_SPEC.md` Module 0 is blunt about the failure mode: "set
//! `repository` correctly the first time. Getting this wrong ships
//! broken links to crates.io." The URL lives once in the workspace
//! `Cargo.toml`, both published crates inherit it, and the changelog's
//! release links use the same base — but nothing enforced that, so a
//! stray edit (or a fork URL) would only be noticed after a publish.
//! The `docs` workflow now re-checks the same invariants on every PR.

mod common;

use common::read_repo_file;

#[test]
fn crates_io_metadata_links_match_the_repository_url() {
    const REPOSITORY_URL: &str = "https://github.com/soroban-testkit/soroban-testkit";

    let workspace_manifest = read_repo_file("Cargo.toml");
    let expected_line = format!("repository = \"{REPOSITORY_URL}\"");
    assert!(
        workspace_manifest.contains(&expected_line),
        "Cargo.toml [workspace.package] must set `{expected_line}` — this is the \
         repository link crates.io shows for both published crates, and it must \
         match the repository's own URL exactly."
    );

    for manifest in ["crates/testkit/Cargo.toml", "crates/cli/Cargo.toml"] {
        let contents = read_repo_file(manifest);
        assert!(
            contents.contains("repository.workspace = true"),
            "{manifest} must inherit the workspace repository URL with \
             `repository.workspace = true`; a hardcoded or missing repository \
             there would drift from {REPOSITORY_URL} on the next metadata edit."
        );
    }

    let changelog = read_repo_file("CHANGELOG.md");
    for (index, line) in changelog.lines().enumerate() {
        let Some(start) = line.find("https://github.com/") else {
            continue;
        };
        let url = &line[start..];
        let aligned = url.strip_prefix(REPOSITORY_URL).is_none_or(|rest| {
            rest.is_empty() || !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
        });
        assert!(
            url.starts_with(REPOSITORY_URL) && aligned,
            "CHANGELOG.md line {} must keep its release links under \
             {REPOSITORY_URL}, but found: {line}",
            index + 1
        );
    }

    let docs_workflow = read_repo_file(".github/workflows/docs.yml");
    assert!(
        docs_workflow.contains(REPOSITORY_URL),
        ".github/workflows/docs.yml must re-check the crates.io metadata against \
         {REPOSITORY_URL} on every PR, so drift is caught before a release rather \
         than discovered on the crates.io page."
    );
}
