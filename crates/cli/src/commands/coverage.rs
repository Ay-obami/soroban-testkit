use std::process::Command;

use clap::{Args, ValueEnum};

use super::CliError;

/// Output format for `soroban-testkit coverage`.
#[derive(Clone, Copy, ValueEnum)]
pub enum Format {
    /// Per-file coverage stats printed to the terminal.
    Text,
    /// An `lcov.info` file, for IDE integrations (e.g. Coverage Gutters).
    Lcov,
    /// A browsable HTML report.
    Html,
}

/// Arguments for `soroban-testkit coverage`.
#[derive(Args)]
pub struct CoverageArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Fail (non-zero exit) if line coverage is below this percentage.
    #[arg(long, value_name = "PCT")]
    fail_under: Option<f64>,
    /// Open the HTML report after generating it (implies --format html).
    #[arg(long)]
    open: bool,
}

/// Wraps `cargo llvm-cov test`, which handles Soroban's coverage needs
/// correctly out of the box — soroban_sdk's own test harness runs contract
/// logic natively (not through the WASM VM), so ordinary LLVM
/// source-based coverage instrumentation applies without any
/// Soroban-specific flags. This command's job is knowing *that*, and
/// knowing the right output-format flags, not reimplementing coverage
/// collection.
///
/// Requires `cargo-llvm-cov` to be installed
/// (`cargo install cargo-llvm-cov`).
pub fn run(args: CoverageArgs) -> Result<(), CliError> {
    let mut cmd = Command::new("cargo");
    cmd.arg("llvm-cov").arg("test");

    match args.format {
        Format::Text => {}
        Format::Lcov => {
            cmd.arg("--lcov").arg("--output-path").arg("lcov.info");
        }
        Format::Html => {
            cmd.arg("--html");
        }
    }

    if args.open {
        if !matches!(args.format, Format::Html) {
            cmd.arg("--html");
        }
        cmd.arg("--open");
    }

    if let Some(pct) = args.fail_under {
        cmd.arg("--fail-under-lines").arg(pct.to_string());
    }

    let status = cmd.status().map_err(|err| {
        CliError(format!(
            "failed to run `cargo llvm-cov` ({err}); is cargo-llvm-cov installed? \
             try `cargo install cargo-llvm-cov`"
        ))
    })?;

    if !status.success() {
        return Err(CliError(format!(
            "coverage run failed (cargo llvm-cov exited with {status})"
        )));
    }

    Ok(())
}
