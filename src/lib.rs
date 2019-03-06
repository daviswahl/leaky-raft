#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene,
    existential_type,
    gen_future,
    box_into_pin
)]
#![allow(unused_imports, dead_code)]
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Error as FmtError, Formatter};

// Just being lazy about exports for now.
pub mod error;
pub mod peer;
pub mod quorum;
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

    pub fn forward<I, E>(
        f: impl old::OldFuture<Item = I, Error = E> + Unpin,
    ) -> impl new::StdFuture<Output = Result<I, E>> {
        use tokio_async_await::compat::forward::IntoAwaitable;
        f.into_awaitable()
    }

    pub type Forward<T> = Compat01As03<T>;

    pub mod old {
        pub use futures_01::{
            Future as OldFuture, IntoFuture as OldIntoFuture, Sink as OldSink, Stream as OldStream,
        };
        pub use futures_util::compat::{Future01CompatExt, Stream01CompatExt};
    }

    pub mod new {
        pub use futures_core::stream::Stream as StdStream;
        pub use futures_util::stream::Unfold;
        pub use futures_util::{
            self as util, FutureExt as StdFutureExt, SinkExt as NewSinkExt,
            StreamExt as StdStreamExt, TryFutureExt as StdTryFutureExt,
            TryStreamExt as StdTryStreamExt,
        };
        pub use std::future::Future as StdFuture;
    }

    use futures_util::compat::Compat;
    use futures_util::compat::Compat01As03;
    pub use futures_util::{self as util, ready};

    pub mod all {
        pub use super::{new::*, old::*, *};
    }
}

pub(crate) fn _debug_old_f<F: futures::old::OldFuture<Item = (), Error = ()>>(_: F) {}

// Like `_debugf` but for `Stream`s instead of `Future`s.
pub(crate) fn _debu_old_s<S: futures::old::OldFuture<Item = (), Error = ()>>(_: S) {}

pub(crate) fn _debug_f<F: futures::new::StdFuture<Output = ()>>(_: F) {}

// Like `_debugf` but for `Stream`s instead of `Future`s.
pub(crate) fn _debug_s<S: futures::new::StdStream<Item = ()>>(_: S) {}

pub use storage::LogIndex;
/// Error type used throughout crate.
pub type Error = error::RaftError;

/// Result type used throughout project.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ServerId(pub std::net::SocketAddr);

impl Display for ServerId {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), FmtError> {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct TermId(pub u64);

impl Display for TermId {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), FmtError> {
        write!(f, "TermId({})", self.0)
    }
}
