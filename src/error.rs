use crate::rpc;
use failure::{Backtrace, Context, Fail};
use std::{fmt, io};

#[derive(Debug)]
pub struct RaftError {
    inner: Context<RaftErrorKind>,
}

impl Fail for RaftError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for RaftError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

#[derive(Debug, Fail)]
pub enum RaftErrorKind {
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

    #[fail(display = "{}", _0)]
    RpcError(#[cause] rpc::RpcError),
}

impl From<RaftErrorKind> for RaftError {
    fn from(kind: RaftErrorKind) -> RaftError {
        RaftError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<RaftErrorKind>> for RaftError {
    fn from(inner: Context<RaftErrorKind>) -> RaftError {
        RaftError { inner: inner }
    }
}

macro_rules! from_error {
    ($i:path, $o:path, $e:expr) => {
        impl From<$i> for RaftErrorKind {
            fn from(_: $i) -> Self {
                $e
            }
        }
    };
    ($i:path, $o:path) => {
        impl From<$i> for RaftErrorKind {
            fn from(e: $i) -> Self {
                $o(e)
            }
        }

        impl From<$i> for RaftError {
            fn from(e: $i) -> Self {
                RaftError {
                    inner: $o(e).into(),
                }
            }
        }
    };
}

from_error!(std::net::AddrParseError, RaftErrorKind::AddrParse);
from_error!(io::Error, RaftErrorKind::Io);
from_error!(sled::Error<()>, RaftErrorKind::SledError);
from_error!(bincode::Error, RaftErrorKind::BincodeError);
from_error!(rpc::RpcError, RaftErrorKind::RpcError);

type StaticStr = &'static str;
from_error!(StaticStr, RaftErrorKind::Lazy);
