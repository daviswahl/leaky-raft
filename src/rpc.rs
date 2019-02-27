use crate::futures::all::*;
use crate::futures::util::compat::Compat01As03;
use crate::futures::util::future::{ready, Ready};
use crate::util::spawn_compat;
use crate::util::RaftError;
use crate::{Result, TermId};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::io;
use std::pin::Pin;
use std::task::LocalWaker;
use std::task::Poll;
use tarpc::context;
use tokio::sync::oneshot::{error::RecvError, Receiver, Sender};

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
        self.response_sender.send(resp).map_err(|_| "rpc".into())
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
        AppendEntriesRep, AppendEntriesReq, RecvError, RequestVoteRep, RequestVoteReq,
        Response as RpcResponse, RpcResult,
    };
    tarpc::service! {
        rpc append_entries(request: AppendEntriesReq) -> AppendEntriesRep;
        rpc request_vote(request: RequestVoteReq) -> RpcResult<RpcResponse>;
    }
}

#[derive(Clone)]
pub struct Server {
    sender: tokio::sync::mpsc::Sender<RequestCarrier>,
}

pub fn new_server(sender: tokio::sync::mpsc::Sender<RequestCarrier>) -> Server {
    Server { sender }
}

pub struct RequestVoteFut {
    inner: Compat01As03<Receiver<Response>>,
}

impl StdFuture for RequestVoteFut {
    type Output = RpcResult<Response>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        self.inner.poll_unpin(lw).map(|p| match p {
            Ok(rep) => Ok(rep),
            _ => Err("woops".into()),
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
        ctx: context::Context,
        request: RequestVoteReq,
    ) -> Self::RequestVoteFut {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let carrier = RequestCarrier {
            response_sender: tx,
            request: Request::RequestVote(request),
        };

        self.sender.start_send(carrier);
        RequestVoteFut { inner: rx.compat() }
    }
}
