#[doc = include_str!("../../../README.md")]

/// See crate-level docs for info.
pub use tangled_actors_macros::make_actor;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[cfg(feature = "eframe")]
pub mod eframe;
mod link;
mod rpcfut;

pub use link::*;
pub use rpcfut::*;

/// Macro internal
#[doc(hidden)]
pub type ReturnChannelSender<T> = oneshot::Sender<T>;

/// Macro internal
#[doc(hidden)]
pub fn oneshot_channel<T>() -> (ReturnChannelSender<T>, oneshot::Receiver<T>) {
    oneshot::channel()
}

/// Actor context. Passed to the actor on creation, can be used by the actor to get link to itself.
pub struct ActorCtx<A: Actor> {
    pub weak_link: WeakLink<A>,
}

impl<A: Actor> ActorCtx<A> {
    // TODO make a special type of link that's guaranteed to stay open while actor is alive.
    pub fn link(&self) -> A::Link {
        self.weak_link.upgrade().expect("actor to still be alive")
    }
}

/// Error type - failed to send the message.
#[derive(Debug, thiserror::Error, Clone)]
#[error("actor is no longer accepting messages")]
pub struct ActorClosed;

/// Error type - sending the message succeeded, but the message handler failed to return a result (panicked) or hasn't been called at all.
#[derive(Debug, thiserror::Error, Clone)]
#[error("no responce from actor (actor's rpc closed return channel) ")]
pub struct NoResponse;

/// Error type - rcp call failed either because actor doesn't accept messages anymore or because it failed to return a result.
#[derive(Debug, thiserror::Error, Clone)]
pub enum RpcError {
    #[error(transparent)]
    ActorClosed(#[from] ActorClosed),
    #[error(transparent)]
    NoResponse(#[from] NoResponse),
}

/// Main trait implemented by actors.
pub trait Actor: Sized + Send + 'static {
    type Message: Send;
    type Link: From<ActorLink<Self>> + Clone + Send;
    /// Spawn an actor task, returning a link object that allows sending messages to this actor.
    fn spawn(
        builder: impl FnOnce(ActorCtx<Self>) -> Self + Send + 'static,
    ) -> (Self::Link, JoinHandle<()>) {
        let (sender, mut r) = mpsc::unbounded_channel();
        let link = ActorLink { sender };
        let weak_link = WeakLink::from(&link);
        let task_handle = tokio::spawn({
            async move {
                let mut actor = builder(ActorCtx { weak_link });
                while let Some(msg) = r.recv().await {
                    actor.process_message(msg).await;
                }
            }
        });
        (Self::Link::from(link), task_handle)
    }
    /// Implemented by macros. See [`ActorSync::process_message_sync`] for a sync version of this.
    fn process_message(
        &mut self,
        message: Self::Message,
    ) -> impl std::future::Future<Output = ()> + Send;
}

/// Extra trait that gets implemented for actors that only have sync actor methods.
pub trait ActorSync: Actor {
    /// Implemented by macros. Sync version of [`Actor::process_message`].
    fn process_message_sync(&mut self, message: Self::Message);
}
