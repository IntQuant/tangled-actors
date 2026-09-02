use std::{
    pin::Pin,
    task::{Poll, ready},
};

use tokio::sync::oneshot;

use crate::{ActorClosed, NoResponse, RpcError};

/// Return type of generated helper methods.
///
/// Implements [`Future`] so can be awaited directly in orded to get [`Result<T, RpcError>`].
///
/// A variety of helper methods like [`Self::away`] and [`Self::resolve`] for non-async contexts are offered as well.
#[must_use]
pub struct RpcFut<T> {
    inner: Result<oneshot::Receiver<T>, ActorClosed>,
}

impl<T> Future for RpcFut<T> {
    type Output = Result<T, RpcError>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        match me.inner.as_mut() {
            Ok(inner) => Poll::Ready(
                ready!(Pin::new(inner).poll(cx)).map_err(|_| RpcError::NoResponse(NoResponse)),
            ),
            Err(ActorClosed) => Poll::Ready(Err(RpcError::ActorClosed(ActorClosed))),
        }
    }
}

impl<T> RpcFut<T> {
    #[doc(hidden)]
    /// Used by macros
    pub fn new(inner: Result<oneshot::Receiver<T>, ActorClosed>) -> Self {
        Self { inner }
    }

    /// Requests an rpc but throws *away* it's result, only returning an error if message wasn't sent.
    pub fn away(self) -> Result<(), ActorClosed> {
        self.inner.map(|_| ())
    }

    /// Block until the result is available and return it.
    ///
    /// Note: .await [`Self`] directly if you need an async version of this.
    pub fn resolve(self) -> Result<T, RpcError> {
        match self.inner {
            Ok(res) => res
                .blocking_recv()
                .map_err(|_| RpcError::NoResponse(NoResponse)),
            Err(err) => Err(err.into()),
        }
    }
}
