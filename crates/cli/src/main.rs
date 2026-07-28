mod commands;

use clap::Parser;
use commands::{audit, coverage, limits, Cli, Commands};

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Coverage(args) => coverage::run(args),
        Commands::Limits(args) => limits::run(args),
        Commands::Audit(args) => audit::run(args),
        Commands::LimitsProbe(args) => limits::run_probe(args),
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
