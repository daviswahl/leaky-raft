use crate::{
    futures::{
        all::*,
        util::future::{ready, Ready},
    },
    server::PersistedState,
    Result, ServerId, TermId,
};
use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use sled::PinnedValue;
use std::path::Path;
use tokio::fs;

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
    type UpdateStateFut = Ready<Result<()>>;
    type ReadStateFut = Ready<Result<Option<PersistedState>>>;
    type AppendLogFut = Ready<Result<()>>;
    type Entry = ();

    fn update_state(&self, state: PersistedState) -> Self::UpdateStateFut {
        if let Ok(state) = serialize(&state) {
            let result = self.db.set(b"state", state).map(|_| ()).map_err(From::from);
            self.db.flush();
            ready(result)
        } else {
            ready(Err("failed to serialize state".into()))
        }
    }

    fn read_state(&self) -> Self::ReadStateFut {
        if let Ok(Some(state)) = self.db.get(b"state") {
            ready(deserialize(&*state).map_err(|e| e.into()))
        } else {
            ready(Ok(None))
        }
    }

    fn append_to_log(&self, entries: &[Self::Entry]) -> Self::AppendLogFut {
        unimplemented!()
    }
}
