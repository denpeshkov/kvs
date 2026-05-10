use std::process::exit;

use clap::*;

#[derive(Parser)]
#[command(version, author, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set the value of a string key to a string
    Set {
        #[arg(help = "A string key")]
        key: String,
        #[arg(help = "The string value of the key")]
        value: String,
    },
    /// Get the string value of a given string key
    Get {
        #[arg(help = "A string key")]
        key: String,
    },
    /// Remove a given key
    Rm {
        #[arg(help = "A string key")]
        key: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Set { key: _, value: _ } => {
            eprintln!("unimplemented");
            exit(1);
        }
        Commands::Get { key: _ } => {
            eprintln!("unimplemented");
            exit(1);
        }
        Commands::Rm { key: _ } => {
            eprintln!("unimplemented");
            exit(1);
        }
    }
}
