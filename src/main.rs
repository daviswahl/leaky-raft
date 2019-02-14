#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene
)]
use futures::{compat::TokioDefaultSpawner, prelude::*};

use env_logger;
use log::{error, info};
use tarpc::server::Handler;
use tarpc_bincode_transport as bincode_transport;

mod client;
mod rpc;
mod server;
mod util;

pub use util::{spawn_compat, RaftError, Result};

static ADDR: &'static str = "0.0.0.0";

async fn spawn_server(port: u32, clients: Vec<u32>) -> Result<()> {
    let port = mk_addr_string(port);
    info!("Spawning server at: {}", port);
    let transport = bincode_transport::listen(&port.parse()?)?;

    let (tx, rx) = tokio::sync::mpsc::channel(1_000);

    // TODO: Keep handle to service so we can shut it down?
    let server = tarpc::server::new(tarpc::server::Config::default())
        .incoming(transport)
        .respond_with(rpc::gen::serve(rpc::new_server(tx)));

    spawn_compat(server);

    let clients = clients.into_iter().map(mk_addr_string).collect();
    let server = server::new(rx, clients)
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
    let futs = vec![
        spawn_server(12000, vec![12001, 12002]),
        spawn_server(12001, vec![12000, 12002]),
        spawn_server(12002, vec![12000, 12001]),
    ]
    .into_iter()
    .map(FutureExt::boxed);
    await!(future::join_all(futs))
        .into_iter()
        .collect::<Result<_>>()?;
    Ok(())
}

fn main() {
    env_logger::init();
    tarpc::init(TokioDefaultSpawner);
    tokio::run(run().map_err(|e| error!("Oh no: {}", e)).boxed().compat());
}
