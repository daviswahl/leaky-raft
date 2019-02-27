#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene,
    existential_type,
    gen_future
)]
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Error as FmtError, Formatter};

// Just being lazy about exports for now.
pub mod client;
pub mod rpc;
pub mod server;
pub mod storage;
pub mod util;

pub mod futures {
    // converts from a new style Future to an old style one:
    pub fn backward<I, E>(
        f: impl new::StdFuture<Output = Result<I, E>>,
    ) -> impl old::OldFuture<Item = I, Error = E> {
        use tokio_async_await::compat::backward;
        backward::Compat::new(f)
    }

    pub mod old {
        pub use futures_01::{
            Future as OldFuture, IntoFuture as OldIntoFuture, Sink as OldSink, Stream as OldStream,
        };
        pub use futures_util::compat::{Future01CompatExt, Stream01CompatExt, TokioDefaultSpawner};
    }

    pub mod new {
        pub use futures_util::{
            self as util, FutureExt as StdFutureExt, StreamExt as StdStreamExt,
            TryFutureExt as StdTryFutureExt, TryStreamExt as StdTryStreamExt,
        };
        pub use std::future::Future as StdFuture;
    }

    pub use futures_util as util;

    pub mod all {
        pub use super::{backward, new::*, old::*};
    }
}

/// Error type used throughout crate.
pub type Error = util::RaftError;

/// Result type used throughout project.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ServerId(pub std::net::SocketAddr);

impl Display for ServerId {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), FmtError> {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct TermId(pub u64);

impl Display for TermId {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), FmtError> {
        write!(f, "TermId({})", self.0)
    }
}
