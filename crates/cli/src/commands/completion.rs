use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, shells::Shell};

use super::CliError;

/// Shell type for completion generation.
#[derive(Clone, Copy, ValueEnum)]
pub enum ShellType {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
}

/// Arguments for `soroban-testkit completion`.
#[derive(Args)]
pub struct CompletionArgs {
    /// The shell to generate completions for.
    #[arg(value_enum)]
    shell: ShellType,
}

pub fn run(args: CompletionArgs) -> Result<(), CliError> {
    let shell = match args.shell {
        ShellType::Bash => Shell::Bash,
        ShellType::Zsh => Shell::Zsh,
        ShellType::Fish => Shell::Fish,
    };

    let mut cmd = crate::commands::Cli::command();
    generate(shell, &mut cmd, "soroban-testkit", &mut std::io::stdout());
    Ok(())
}
