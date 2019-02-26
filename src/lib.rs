#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene,
    existential_type
)]
use serde::{Deserialize, Serialize};

// Just being lazy about exports for now.
pub mod client;
pub mod rpc;
pub mod server;
pub mod storage;
pub mod util;

/// Error type used throughout crate.
pub type Error = util::RaftError;

/// Result type used throughout project.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ServerId(pub std::net::SocketAddr);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct TermId(pub usize);
