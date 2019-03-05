use crate::{
    futures::{all::*, util::future},
    server::PersistedState,
    Result,
};
use bincode;
use serde::{Deserialize, Serialize};

use crate::error::RaftResultExt;
use std::path::Path;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct LogIndex(u64);
impl LogIndex {
    pub fn new(u: u64) -> LogIndex {
       assert!(u > 0, "LogIndex must be > 0");
        LogIndex(u)
    }
}
/// Interface for async storage adapter
pub trait Storage {
    type UpdateStateFut: StdFuture<Output = Result<()>>;
    type ReadStateFut: StdFuture<Output = Result<Option<PersistedState>>>;
    type AppendLogFut: StdFuture<Output = Result<()>>;

    type Entry: Serialize + for<'a> Deserialize<'a>;

    fn update_state(&self, state: PersistedState) -> Self::UpdateStateFut;
    fn read_state(&self) -> Self::ReadStateFut;
    fn append_to_log(&self, entries: &[Self::Entry]) -> Self::AppendLogFut;
}

pub struct SledStorage {
    db: sled::Db,
}

impl SledStorage {
    pub fn new<P: AsRef<Path>>(p: P) -> Result<SledStorage> {
        let db = sled::Db::start_default(p)?;
        Ok(SledStorage { db })
    }
}

impl Storage for SledStorage {
    type AppendLogFut = future::Ready<Result<()>>;
    type Entry = ();

    type UpdateStateFut = future::Ready<Result<()>>;
    fn update_state(&self, state: PersistedState) -> Self::UpdateStateFut {
        if let Ok(state) = bincode::serialize(&state) {
            let result = self
                .db
                .set(b"state", state)
                .and_then(|_| self.db.flush())
                .into_raft_result();
            future::ready(result)
        } else {
            future::ready(Err("failed to serialize state".into()))
        }
    }

    type ReadStateFut = future::Ready<Result<Option<PersistedState>>>;
    fn read_state(&self) -> Self::ReadStateFut {
        if let Ok(Some(state)) = self.db.get(b"state") {
            future::ready(bincode::deserialize(&*state).map(Some).into_raft_result())
        } else {
            future::ready(Ok(None))
        }
    }

    fn append_to_log(&self, _entries: &[Self::Entry]) -> Self::AppendLogFut {
        unimplemented!()
    }
}
