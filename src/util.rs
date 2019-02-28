use crate::futures::new::*;
use failure::Fail;

use std::io;

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
