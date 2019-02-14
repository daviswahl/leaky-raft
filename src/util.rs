use failure::Fail;
use futures::prelude::*;
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
}

macro_rules! from_error {
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

type StaticStr = &'static str;
from_error!(StaticStr, RaftError::Lazy);

pub type Result<T> = ::std::result::Result<T, RaftError>;

pub fn spawn_compat<F: Future<Output = ()> + Send + 'static>(fut: F) {
    let fut = fut.boxed().unit_error().compat();
    tokio_executor::spawn(fut)
}
