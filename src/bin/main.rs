#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene
)]

#[macro_use]
extern crate leaky_raft;

use futures::{compat::TokioDefaultSpawner, prelude::*};
use leaky_raft::util::spawn_compat;

use env_logger;
use log::{error, info};
use tarpc::server::Handler;
use tarpc_bincode_transport as bincode_transport;

use leaky_raft::storage::SledStorage;
use leaky_raft::ServerId;
use leaky_raft::{rpc, server, Error, Result};
use std::path::Path;
use std::path::PathBuf;
// Basically everything ./main.rs is temporary.

static ADDR: &'static str = "0.0.0.0";
static TMP: &'static str = "/tmp/leaky-raft";

async fn spawn_server(port: u32, clients: Vec<u32>) -> Result<()> {
    let port = mk_addr_string(port);

    let addr = port.parse()?;

    let clients = clients.into_iter().map(mk_addr_string).collect();

    let root = port.replace(".", "_").replace(":", "_");
    let root = Path::new(TMP).join(root);

    let storage = SledStorage::new(root.join("sled"))?;
    let server = await!(server::new(addr, clients, storage))?
        .start()
        .map(|complete| match complete {
            Ok(state) => info!("stream exhausted: {}", state.cycles),
            Err(e) => error!("{}", e),
        });

    spawn_compat(server);
    Ok(())
}

fn mk_addr_string(port: u32) -> String {
    format!("{}:{}", ADDR, port)
}

async fn run() -> Result<()> {
    collect_await!(
        spawn_server(12000, vec![12001, 12002]),
        spawn_server(12001, vec![12000, 12002]),
        spawn_server(12002, vec![12000, 12001]),
    )?;

    Ok(())
}

fn main() -> Result<()> {
    use std::fs;
    fs::remove_dir_all::<PathBuf>(TMP.into()).unwrap_or(());
    fs::create_dir::<PathBuf>(TMP.into())?;
    env_logger::init();
    tarpc::init(TokioDefaultSpawner);
    tokio::run(run().map_err(|e| error!("Oh no: {}", e)).boxed().compat());
    Ok(())
}
