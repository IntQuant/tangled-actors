use std::{
    pin::Pin,
    task::{Poll, ready},
};

use tokio::sync::oneshot;

use crate::{ActorClosed, NoResponse, RpcError};

/// Return type of generated helper methods.
///
/// Implements [`Future`] so can be awaited directly in orded to get `T`.
///
/// A variety of helper methods like [`Self::away`] and [`Self::resolve`] for non-async contexts are offered as well.
///
/// ## Note on panic behavior
///
/// Actor RPCs can only fail in case an actor panics:
/// - Actors only get closed when last link to them gets dropped, so getting [`ActorClosed`] is impossible unless a panic happens.
/// - Message handlers are guaranteed to return the value when they finish, and thus [`NoResponse`] is impossible to get unless they panic.
///
/// Given how exceptional those situations are, [`RpcFut`] simply unwraps these errors by default.
/// `try_` series of methods are offered in case you wish to handle these errors anyway.
///
#[must_use]
pub struct RpcFut<T> {
    inner: Result<oneshot::Receiver<T>, ActorClosed>,
}

impl<T> Future for RpcFut<T> {
    type Output = T;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        match me.inner.as_mut() {
            Ok(inner) => Poll::Ready(
                ready!(Pin::new(inner).poll(cx))
                    .map_err(|_| RpcError::NoResponse(NoResponse))
                    .expect("actor to respond to rpc"),
            ),
            Err(ActorClosed) => Poll::Ready(
                Err(RpcError::ActorClosed(ActorClosed)).expect("actor to not be closed"),
            ),
        }
    }
}

impl<T> RpcFut<T> {
    #[doc(hidden)]
    /// Used by macros
    pub fn new(inner: Result<oneshot::Receiver<T>, ActorClosed>) -> Self {
        Self { inner }
    }

    /// Requests an rpc but throws *away* it's result, panicking if message wasn't sent.
    pub fn away(self) {
        self.try_away().expect("actor should not be closed")
    }

    /// Requests an rpc but throws *away* it's result, only returning an error if message wasn't sent.
    pub fn try_away(self) -> Result<(), ActorClosed> {
        self.inner.map(|_| ())
    }

    /// Block until the result is available and returns it.
    ///
    /// Panics if communication with the actor failed. See [`Self::try_resolve`] if you'd like to handle this
    ///
    /// Note: .await [`Self`] directly if you need an async version of this.
    pub fn resolve(self) -> T {
        self.try_resolve().expect("actor hasn't panicked")
    }

    /// Non-panicking version of [`Self::resolve`].
    pub fn try_resolve(self) -> Result<T, RpcError> {
        match self.inner {
            Ok(res) => res
                .blocking_recv()
                .map_err(|_| RpcError::NoResponse(NoResponse)),
            Err(err) => Err(err.into()),
        }
    }

    /// Async version of [`Self::try_resolve`].
    pub async fn try_resolve_async(self) -> Result<T, RpcError> {
        match self.inner {
            Ok(res) => res.await.map_err(|_| RpcError::NoResponse(NoResponse)),
            Err(err) => Err(RpcError::ActorClosed(err)),
        }
    }
}
