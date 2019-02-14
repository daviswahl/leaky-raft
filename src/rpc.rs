use futures::{future, future::Ready};
use serde::{Deserialize, Serialize};
use tarpc::context;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    AppendEntries(AppendEntriesRep),
    RequestVote(RequestVoteRep),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
    AppendEntries(AppendEntriesReq),
    RequestVote(RequestVoteReq),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppendEntriesReq {}
#[derive(Debug, Deserialize, Serialize)]
pub struct AppendEntriesRep {}

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestVoteReq {}
#[derive(Debug, Deserialize, Serialize)]
pub struct RequestVoteRep {}

pub struct RequestCarrier {
    _response_sender: Sender<Response>,
    _request: Request,
}

pub mod gen {
    use super::{AppendEntriesRep, AppendEntriesReq, RequestVoteRep, RequestVoteReq};
    tarpc::service! {
        rpc append_entries(request: AppendEntriesReq) -> AppendEntriesRep;
        rpc request_vote(request: RequestVoteReq) -> RequestVoteRep;
    }
}

#[derive(Clone)]
pub struct Server {
    sender: tokio::sync::mpsc::Sender<RequestCarrier>,
}

pub fn new_server(sender: tokio::sync::mpsc::Sender<RequestCarrier>) -> Server {
    Server { sender }
}

impl gen::Service for Server {
    type AppendEntriesFut = Ready<AppendEntriesRep>;
    type RequestVoteFut = Ready<RequestVoteRep>;

    fn append_entries(self, _ctx: context::Context, _request: AppendEntriesReq) -> Self::AppendEntriesFut {
        future::ready(AppendEntriesRep {})
    }

    fn request_vote(self, _ctx: context::Context, _request: RequestVoteReq) -> Self::RequestVoteFut {
        future::ready(RequestVoteRep {})
    }
}
