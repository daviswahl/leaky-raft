use crate::futures::new::*;
use failure::Fail;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::io;

#[derive(Fail, Debug)]
pub enum RaftError {
    #[fail(display = "Server error: {}", _0)]
    ServerError(&'static str),
    #[fail(display = "{}", _0)]
    Io(#[cause] io::Error),

    #[fail(display = "{}", _0)]
    AddrParse(#[cause] std::net::AddrParseError),

    #[fail(display = "{}", _0)]
    Lazy(&'static str),

    #[fail(display = "{}", _0)]
    SledError(#[cause] sled::Error<()>),

    #[fail(display = "{}", _0)]
    BincodeError(#[cause] bincode::Error),
}

macro_rules! from_error {
    ($i:path, $o:path, $e:expr) => {
        impl From<$i> for RaftError {
            fn from(_: $i) -> Self {
                $e
            }
        }
    };
    ($i:path, $o:path) => {
        impl From<$i> for RaftError {
            fn from(e: $i) -> Self {
                $o(e)
            }
        }
    };
}

from_error!(std::net::AddrParseError, RaftError::AddrParse);
from_error!(io::Error, RaftError::Io);
from_error!(sled::Error<()>, RaftError::SledError);
from_error!(bincode::Error, RaftError::BincodeError);

type StaticStr = &'static str;
from_error!(StaticStr, RaftError::Lazy);

/// Convenience function for spawning a Future03 on the tokio executor.
pub fn spawn_compat<F: StdFuture<Output = ()> + Send + 'static>(fut: F) {
    let fut = fut.boxed().unit_error().compat();
    tokio_executor::spawn(fut)
}

/// Collect a vec or comma-separated list of Future<Output=Result<_>> into a Result<Vec<_>>.
/// If any Future returns an Err, the entire result is Err.
#[macro_export]
macro_rules! collect_await {
    ($e:expr) => {
        await!(::futures::future::join_all(
            $e.into_iter().map(::futures_util::FutureExt::boxed)
        ))
        .into_iter()
        .collect::<Result<Vec<_>>>()
    };
    ($($x:expr),*) => (
        collect!(<[_]>::into_vec(box [$($x),*]))
    );
    ($($x:expr,)*) => (collect_await!(vec![$($x),*]));
}
