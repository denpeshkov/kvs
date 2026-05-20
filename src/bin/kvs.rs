use std::env;

use clap::{Parser, Subcommand};
use kvs::{KvStore, KvsError, Result};

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut kvs = KvStore::open(env::current_dir()?)?;
    match cli.command {
        Commands::Set { key, value } => kvs.set(key, value),
        Commands::Get { key } => {
            if let Some(value) = kvs.get(key)? {
                println!("{value}");
            } else {
                println!("Key not found");
            }
            Ok(())
        }
        Commands::Rm { key } => match kvs.remove(key) {
            Ok(()) => Ok(()),
            Err(err @ KvsError::KeyNotFound) => {
                println!("Key not found");
                Err(err)
            }
            Err(e) => Err(e),
        },
    }
}
