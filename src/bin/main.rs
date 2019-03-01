#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene
)]
/// Just a temporary executable for testing

#[macro_use]
extern crate leaky_raft;

use leaky_raft::util::spawn_compat;

use env_logger;

use log::{error, info};

use leaky_raft::storage::SledStorage;

use leaky_raft::{futures::all::*, server, Result};

use std::path::Path;
use std::path::PathBuf;

static ADDR: &'static str = "0.0.0.0";
static TMP: &'static str = "/tmp/leaky-raft";

async fn spawn_server(port: u32, clients: Vec<u32>) -> Result<()> {
    let port = mk_addr_string(port);

    let addr = port.parse()?;

    let clients = clients.into_iter().map(mk_addr_string).collect();

    let root = port.replace(".", "_").replace(":", "_");
    let root = Path::new(TMP).join(root);

    let storage = SledStorage::new(root.join("sled"))?;
    let server =
        await!(server::new(addr, clients, storage))?
            .start()
            .map(|complete| match complete {
                Ok(()) => info!("stream exhausted"),
                Err(e) => error!("{}", e),
            });

    spawn_compat(server.shared());
    Ok(())
}

fn mk_addr_string(port: u32) -> String {
    format!("{}:{}", ADDR, port)
}

async fn spawn_3() -> Result<()> {
    collect_await!(
        spawn_server(12000, vec![12001, 12002]),
        spawn_server(12001, vec![12000, 12002]),
        spawn_server(12002, vec![12000, 12001]),
    )?;
    Ok(())
}
async fn run() -> Result<()> {
    collect_await!(spawn_server(12000, vec![]),)?;

    Ok(())
}

fn main() -> Result<()> {
    use failure::Fail;
    use futures_util::compat::Executor01CompatExt;
    use std::fs;
    fs::remove_dir_all::<PathBuf>(TMP.into()).unwrap_or(());
    fs::create_dir::<PathBuf>(TMP.into())?;
    env_logger::init();
    tarpc::init(tokio::executor::DefaultExecutor::current().compat());
    tokio::run(
        run()
            .map_err(|e| error!("Oh no: {:?}", e.backtrace()))
            .boxed()
            .compat(),
    );
    Ok(())
}
