use std::{
    env, fs,
    io::{self},
    net::SocketAddr,
    process, str,
};

use clap::{Parser, ValueEnum};
use kvs::{Error, engines, proto};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "kvs-server", version, author,about, long_about = None)]
struct Cli {
    /// Address to listen on (format: IP:PORT).
    #[arg(long, default_value = "127.0.0.1:4000")]
    addr: SocketAddr,

    /// Storage engine to use.
    #[arg(long)]
    engine: Option<Engine>,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum Engine {
    #[value(name = "kvs")]
    Kvs,
    #[value(name = "sled")]
    Sled,
}

impl str::FromStr for Engine {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(Error::MalformedMetadata)
    }
}

fn main() -> kvs::Result<()> {
    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let cli = Cli::parse();

    let meta_path = env::current_dir()?.join("storage");
    let persisted_engine: Option<Engine> =
        if meta_path.exists() { Some(fs::read_to_string(&meta_path)?.parse()?) } else { None };

    let engine = match cli.engine {
        Some(e) => {
            if let Some(pe) = persisted_engine
                && pe != e
            {
                eprintln!("Error: Data was previously persisted with a different engine");
                process::exit(1);
            }
            e
        }
        None => persisted_engine.map_or(Engine::Kvs, |pe| pe),
    };
    fs::write(meta_path, format!("{engine:?}"))?;

    info!(?engine, ?cli.addr, version = %env!("CARGO_PKG_VERSION"));

    let kvs: &mut dyn engines::KvsEngine = match engine {
        Engine::Kvs => &mut kvs::engines::KvStore::open(env::current_dir()?)?,
        Engine::Sled => &mut kvs::engines::SledStore::open(env::current_dir()?)?,
    };

    let ln = std::net::TcpListener::bind(cli.addr)?;
    for stream in ln.incoming() {
        let stream = stream?;

        info!(local_addr = %stream.local_addr()?, peer_addr = %stream.peer_addr()?);

        let (req, _): (proto::Request, _) = postcard::from_io((&stream, &mut [0; 1024]))?;

        let resp = match req {
            proto::Request::Set { key, value } => {
                proto::Response::Set(proto::Result(kvs.set(key, value).map_err(|e| e.to_string())))
            }
            proto::Request::Rm { key } => {
                proto::Response::Rm(proto::Result(kvs.remove(key).map_err(|e| e.to_string())))
            }
            proto::Request::Get { key } => {
                proto::Response::Get(proto::Result(kvs.get(key).map_err(|e| e.to_string())))
            }
        };
        postcard::to_io(&resp, stream)?;
    }
    Ok(())
}
