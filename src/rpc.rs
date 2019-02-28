use crate::futures::all::*;
use crate::futures::util::compat::Compat01As03;
use crate::futures::util::future::{ready, Ready};

use crate::util::RaftError;
use crate::{Result, TermId};
use serde::{Deserialize, Serialize};
use std::io;
use std::pin::Pin;
use std::sync::mpsc::SendError;
use std::task::Poll;
use std::task::Waker;
use tarpc::context;
use tokio::prelude::Async;
use tokio::prelude::AsyncSink;
use tokio::prelude::Sink;
use tokio::sync::mpsc;
use tokio::sync::oneshot::{Receiver, Sender};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Response {
    AppendEntries(AppendEntriesRep),
    RequestVote(RequestVoteRep),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Request {
    AppendEntries(AppendEntriesReq),
    RequestVote(RequestVoteReq),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppendEntriesReq {}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppendEntriesRep {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestVoteReq {}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestVoteRep {
    pub term: TermId,
    pub vote_granted: bool,
}

#[derive(Debug)]
pub struct RequestCarrier {
    response_sender: Sender<Response>,
    request: Request,
}

impl RequestCarrier {
    pub fn body(&self) -> &Request {
        &self.request
    }
    pub async fn respond(self, resp: Response) -> Result<()> {
        if !self.response_sender.is_closed() {
            self.response_sender.send(resp).map_err(|_| "rpc".into())
        } else {
            Err("sender is closed".into())
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcError {}
pub type RpcResult<T> = std::result::Result<T, RpcError>;

impl From<RaftError> for RpcError {
    fn from(_: RaftError) -> Self {
        RpcError {}
    }
}

impl From<io::Error> for RpcError {
    fn from(_: io::Error) -> Self {
        RpcError {}
    }
}

impl From<&'static str> for RpcError {
    fn from(_: &'static str) -> Self {
        RpcError {}
    }
}

pub mod gen {
    use super::{
        AppendEntriesRep, AppendEntriesReq, RequestVoteReq, Response as RpcResponse, RpcResult,
    };
    tarpc::service! {
        rpc append_entries(request: AppendEntriesReq) -> AppendEntriesRep;
        rpc request_vote(request: RequestVoteReq) -> RpcResult<RpcResponse>;
    }
}

struct RequestSender<T>(tokio::sync::mpsc::Sender<T>);
impl<T> Clone for RequestSender<T> {
    fn clone(&self) -> Self {
        RequestSender(self.0.clone())
    }
}

impl<T> Drop for RequestSender<T> {
    fn drop(&mut self) {}
}

impl<T> Sink for RequestSender<T> {
    type SinkItem = T;
    type SinkError = mpsc::error::SendError;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> std::result::Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        self.0.start_send(item)
    }

    fn poll_complete(&mut self) -> std::result::Result<Async<()>, Self::SinkError> {
        self.0.poll_complete()
    }

    fn close(&mut self) -> std::result::Result<Async<()>, Self::SinkError> {
        self.0.close()
    }
}

#[derive(Clone)]
pub struct Server {
    sender: RequestSender<RequestCarrier>,
}

pub fn new_server(sender: tokio::sync::mpsc::Sender<RequestCarrier>) -> Server {
    Server {
        sender: RequestSender(sender),
    }
}

pub struct RequestVoteFut {
    inner: Compat01As03<Receiver<Response>>,
}

impl StdFuture for RequestVoteFut {
    type Output = RpcResult<Response>;

    fn poll(mut self: Pin<&mut Self>, lw: &Waker) -> Poll<Self::Output> {
        log::debug!("polling receive vote fut");
        self.inner.poll_unpin(lw).map(|p| match p {
            Ok(rep) => Ok(rep),
            Err(e) => Err("RecvError".into()),
        })
    }
}

impl gen::Service for Server {
    type AppendEntriesFut = Ready<AppendEntriesRep>;
    type RequestVoteFut = RequestVoteFut;

    fn append_entries(
        self,
        _ctx: context::Context,
        _request: AppendEntriesReq,
    ) -> Self::AppendEntriesFut {
        ready(AppendEntriesRep {})
    }

    fn request_vote(
        mut self,
        _ctx: context::Context,
        request: RequestVoteReq,
    ) -> Self::RequestVoteFut {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let carrier = RequestCarrier {
            response_sender: tx,
            request: Request::RequestVote(request),
        };

        use log::error;
        match self.sender.start_send(carrier) {
            Err(e) => {
                error!("request vote: {}", e);
            }
            _ => (),
        }
        RequestVoteFut { inner: rx.compat() }
    }
}
