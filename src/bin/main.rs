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

use clap::{App, Arg, SubCommand};
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
async fn run(port: u32, peers: Vec<u32>) -> Result<()> {
    await!(spawn_server(port, peers))?;

    Ok(())
}

fn main() -> Result<()> {
    use failure::Fail;
    use futures_util::compat::Executor01CompatExt;
    use std::fs;

    env_logger::init();
    let matches = App::new("leaky-raft test")
        .version("1.0")
        .author("Davis W. <daviswahl@gmail.com>")
        .about("Does awesome things")
        .arg(
            Arg::with_name("PORT")
                .help("port to use")
                .short("p")
                .takes_value(true),
        )
        .arg(Arg::with_name("TMP").help("remake tmp dir").short("r"))
        .arg(
            Arg::with_name("PEERS")
                .help("comma separated peer ports")
                .short("P")
                .value_delimiter(","),
        )
        .get_matches();

    let port: u32 = matches.value_of("PORT").unwrap().parse().unwrap();

    let peers: Vec<u32> = matches
        .values_of("PEERS")
        .unwrap()
        .map(|p| p.parse().unwrap())
        .collect();

    let remake = matches.is_present("TMP");
    if remake {
        log::info!("removing tmp dir");
        fs::remove_dir_all::<PathBuf>(TMP.into()).unwrap_or(());
    }

    fs::create_dir::<PathBuf>(TMP.into()).unwrap_or(());
    tarpc::init(tokio::executor::DefaultExecutor::current().compat());
    tokio::run(
       spawn_3()
            .map_err(|e| error!("Oh no: {:?}", e))
            .boxed()
            .compat(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::compat::Executor01CompatExt;
    use leaky_raft::{futures::all::*, server, Result};
    use std::fs;
    use std::process::Command;
    use std::process::Stdio;

    #[test]
    fn test_main_1() {
        let mut cmd = Command::new("/code/leaky-raft/target/debug/main");
        cmd.env("RUST_LOG", "info")
            .args(&["-p=13005", "-P=13004,13003"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        assert_eq!(output.status.success(), true);
    }
}
