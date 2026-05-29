use std::net::{SocketAddr, TcpStream};

use clap::{Parser, Subcommand};
use kvs::proto::{self, Request};

#[derive(Parser, Debug)]
#[command(name = "kvs-client", version, author, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Set the value of a string key to a string.
    Set {
        /// A string key.
        key: String,

        /// The string value of the key.
        value: String,

        /// Address to connect to (format: IP:PORT).
        #[arg(long, default_value = "127.0.0.1:4000")]
        addr: SocketAddr,
    },

    /// Get the string value of a given string key.
    Get {
        /// A string key.
        key: String,

        /// Address to connect to (format: IP:PORT).
        #[arg(long, default_value = "127.0.0.1:4000")]
        addr: SocketAddr,
    },

    /// Remove a given key.
    Rm {
        /// A string key.
        key: String,

        /// Address to connect to (format: IP:PORT).
        #[arg(long, default_value = "127.0.0.1:4000")]
        addr: SocketAddr,
    },
}

fn main() -> kvs::Result<()> {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();

    let (req, addr) = match cli.command {
        Commands::Set { key, value, addr } => (Request::Set { key, value }, addr),
        Commands::Rm { key, addr } => (Request::Rm { key }, addr),
        Commands::Get { key, addr } => (Request::Get { key }, addr),
    };

    let stream = TcpStream::connect(addr)?;
    postcard::to_io(&req, &stream)?;
    let (resp, _): (proto::Response, _) = postcard::from_io((&stream, &mut [0; 1024]))?;

    match resp {
        proto::Response::Set(res) | proto::Response::Rm(res) => res.0.map_err(kvs::Error::Server),
        proto::Response::Get(res) => {
            match res.0.map_err(kvs::Error::Server)? {
                Some(v) => println!("{v}"),
                None => println!("Key not found"),
            }
            Ok(())
        }
    }
}
