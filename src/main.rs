use clap::Parser;
use netsuke::cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Build { targets: vec![] }) {
        Commands::Build { targets } => {
            println!("Building targets: {targets:?}");
        }
        Commands::Clean {} => {
            println!("Clean requested");
        }
        Commands::Graph {} => {
            println!("Graph requested");
        }
    }
}
