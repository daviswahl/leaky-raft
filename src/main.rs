#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene
)]
use futures::{compat::TokioDefaultSpawner, prelude::*};

use tarpc::server::Handler;
use tarpc_bincode_transport as bincode_transport;

mod client;
mod rpc;
mod server;
mod util;

pub use util::{spawn_compat, Result};

async fn spawn_server(port: u32, clients: Vec<u32>) -> Result<()> {
    let port = format!("0.0.0.0:{}", port);
    println!("Spawning server at: {}", port);
    let transport = bincode_transport::listen(&port.parse()?)?;

    let (tx, rx) = tokio::sync::mpsc::channel(1_000);
    let server = tarpc::server::new(tarpc::server::Config::default())
        .incoming(transport)
        .respond_with(rpc::gen::serve(rpc::new_server(tx)));

    spawn_compat(server);

    let server = server::new(rx, clients)
        .start()
        .map(|complete| match complete {
            Ok(state) => eprintln!("stream exhausted: {}", state.cycles),
            Err(e) => eprintln!("{}", e),
        });

    spawn_compat(server);
    Ok(())
}

async fn run() -> Result<()> {
    // TODO: Spawn these concurrently
    await!(spawn_server(12000, vec![12001, 12002]))?;
    await!(spawn_server(12001, vec![12000, 12002]))?;
    await!(spawn_server(12002, vec![12000, 12001]))?;
    Ok(())
}

fn main() {
    tarpc::init(TokioDefaultSpawner);
    tokio::run(
        run()
            .map_err(|e| eprintln!("Oh no: {}", e))
            .boxed()
            .compat(),
    );
}
