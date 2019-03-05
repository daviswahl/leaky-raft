use crate::rpc::RequestVoteReq;
use crate::rpc::Response;
use crate::rpc::RpcError;
use crate::rpc::RpcResult;
use crate::util::spawn_compat;
use crate::{futures::all::*, rpc, Result, ServerId, TermId, LogIndex};

use std::net::SocketAddr;
use std::pin::Pin;

use std::sync::Arc;
use std::task::Poll;
use std::task::Waker;
use tarpc_bincode_transport as bincode_transport;

use crate::rpc::RequestVoteRep;
use core::mem;
use futures_util::future::RemoteHandle;
use std::future::poll_with_tls_waker;
use tokio::prelude::Async;
use tokio::sync::oneshot;

pub struct Quorum {
    parent: ServerId,
    peers: Vec<Peer>,
    receiver: Option<RemoteHandle<Result<()>>>,
}

fn iter_pin_mut<T>(slice: Pin<&mut [T]>) -> impl Iterator<Item = Pin<&mut T>> {
    // Safety: `std` _could_ make this unsound if it were to decide Pin's
    // invariants aren't required to transmit through slices. Otherwise this has
    // the same safety as a normal field pin projection.
    unsafe { slice.get_unchecked_mut() }
        .iter_mut()
        .map(|t| unsafe { Pin::new_unchecked(t) })
}
impl Quorum {
    pub fn new<R: AsRef<str>>(parent: ServerId, peers: Vec<R>) -> Self {
        Quorum {
            parent,
            peers: peers.iter().map(Peer::new).map(|e| e.unwrap()).collect(),
            receiver: None,
        }
    }

    pub fn request_vote(&mut self, server: ServerId, term: TermId, last_log_index: crate::LogIndex, last_term_id: TermId) -> Result<()> {
        if let Some(mut rx) = self.receiver.take() {
            log::debug!("{}, quorum: rx exists", self.parent);
        }

        let fut = self
            .peers
            .clone()
            .into_iter()
            .map(|peer| peer.request_vote(server, term, last_log_index, last_term_id).map(|e| e.map_err(|e| e.into())));

        //        let stream = util::stream::futures_unordered(fut);
        //        let _stream = stream
        //            .try_filter_map(|res| {
        //                async {
        //                    match res {

        //                        rpc::Response::RequestVote(rep) => {
        //                            if rep.vote_granted {
        //                                Ok(Some(true))
        //                            } else {
        //                                Ok(None)
        //                            }
        //                        }
        //                        _ => Ok(None),
        //                    }
        //                }
        //            })
        //            .try_collect()
        //            .map(|vec: rpc::RpcResult<Vec<_>>| {
        //                if let Ok(v) = vec {
        //                    tx.send(v).unwrap_or_else(|e| log::error!("error: {:?}", e))
        //                }
        //            });
        let (remote, handle) = VoteResult::new(fut).remote_handle();
        spawn_compat(remote);
        self.receiver.replace(handle);

        //spawn_compat(stream);
        Ok(())
    }

    pub fn poll_response(&mut self) -> Result<()> {
        if let Some(ref mut recv) = self.receiver {
            let recv = poll_with_tls_waker(Pin::new(recv));
            match recv {
                Poll::Ready(Ok(_)) => {
                    self.receiver.take();
                    Ok(log::debug!("got resp"))
                }
                Poll::Ready(Err(_e)) => {
                    self.receiver.take();
                    Err("RecvError in Quorum::poll_response".into())
                }
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }
}

struct VoteResult<F: StdFuture> {
    elems: Pin<Box<[ElemState<F>]>>,
}

enum ElemState<F: StdFuture> {
    Pending(F),
    Complete(Option<F::Output>),
}

impl<F> ElemState<F>
where
    F: StdFuture<Output = Result<rpc::Response>>,
{
    fn pending_pin_mut<'a>(self: Pin<&'a mut Self>) -> Option<Pin<&'a mut F>> {
        // Safety: Basic enum pin projection, no drop + optionally Unpin based
        // on the type of this variant
        match unsafe { self.get_unchecked_mut() } {
            ElemState::Pending(f) => Some(unsafe { Pin::new_unchecked(f) }),
            ElemState::Complete(_) => None,
        }
    }

    fn take_done(self: Pin<&mut Self>) -> Option<F::Output> {
        // Safety: Going from pin to a variant we never pin-project
        match unsafe { self.get_unchecked_mut() } {
            ElemState::Pending(_) => None,
            ElemState::Complete(output) => output.take(),
        }
    }
}

impl<F> StdFuture for VoteResult<F>
where
    F: StdFuture<Output = Result<rpc::Response>>,
{
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let mut all_done = true;

        for mut elem in iter_pin_mut(self.elems.as_mut()) {
            if let Some(pending) = elem.as_mut().pending_pin_mut() {
                if let Poll::Ready(output) = pending.poll(waker) {
                    elem.set(ElemState::Complete(Some(output)));
                } else {
                    all_done = false;
                }
            }
        }

        if all_done {
            let mut elems = mem::replace(&mut self.elems, Box::pin([]));
            let result: Vec<_> = iter_pin_mut(elems.as_mut())
                .map(|e| e.take_done().unwrap())
                .collect();
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl<F> VoteResult<F>
where
    F: StdFuture,
{
    fn new<I: IntoIterator<Item = F>>(futs: I) -> Self
    where
        F: StdFuture<Output = Result<rpc::Response>> + Send + 'static,
    {
        let elems: Box<[_]> = futs.into_iter().map(ElemState::Pending).collect();
        VoteResult {
            elems: Box::into_pin(elems),
        }
    }
}

#[derive(Clone)]
pub struct Peer {
    addr: Arc<SocketAddr>,
    connection: Option<rpc::gen::Client>,
}

impl Peer {
    fn new<R: AsRef<str>>(addr: R) -> Result<Peer> {
        let addr = addr.as_ref().parse()?;
        Ok(Peer {
            addr: Arc::new(addr),
            connection: None,
        })
    }
    /// If we have an established connection, clone and return it. Otherwise,
    /// establish the connection, clone and return it,
    ///
    /// I'm pretty sure cloning the underlying client is right. Could lock it
    /// instead but it's already synchronized internally so no point.
    async fn connect(&mut self) -> Result<rpc::gen::Client> {
        match self.connection {
            Some(ref conn) => Ok(conn.clone()),
            None => {
                let transport = await!(bincode_transport::connect(&self.addr))?;
                let client = await!(crate::rpc::gen::new_stub(
                    tarpc::client::Config::default(),
                    transport
                ))?;
                self.connection.replace(client.clone());
                Ok(client)
            }
        }
    }

    pub async fn request_vote(mut self, candidate: ServerId, term: TermId, last_log_index: LogIndex, last_log_term: TermId) -> rpc::RpcResult<rpc::Response> {
        let mut conn = await!(self.connect())?;
        await!(conn
            .request_vote(tarpc::context::current(), rpc::RequestVoteReq { candidate, term, last_log_index, last_log_term })
            .err_into::<RpcError>()
            .map(|e| e.unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        tokio::runtime::current_thread::run(backward(
            async {
                let mut q = Quorum::new(
                    ServerId("127.0.0.1:1233".parse().unwrap()),
                    vec!["127.0.0.1:1234"],
                );
                q.request_vote(ServerId("127.0.0.1:1234".parse().unwrap()), TermId(0));
                Ok(())
            },
        ))
    }
}
