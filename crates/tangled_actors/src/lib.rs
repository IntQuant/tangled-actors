//! # tangled_actors
//!
//! `tangled_actors` is yet another actor crate with excessive use of macros.
//!
//! [`make_actor`] macro is used on an impl block to generate an [`Actor`] trait impl, a message enum,
//! and an "actor link" struct with rpc-style methods.
//! Notably, every generated method inherits visibility of the original function.
//!
//! Actors are spawned using [`Actor::spawn`] trait method.
//!
//! ## Example
//!
//! ```rust
//! use tangled_actors::{Actor, make_actor};
//!
//! struct Counter {
//!     count: u32,
//! }
//!
//! // Separate impl block can be used for non-actor methods.
//! impl Counter {
//!     fn new() -> Self {
//!         Self { count: 0 }
//!     }
//! }
//!
//! // Every function defined here will be turned into actor methods.
//! #[make_actor]
//! impl Counter {
//!     fn increment(&mut self, amount: u32) {
//!         self.count += amount;
//!     }
//!
//!     // `get_count` on generated CounterLink type will be public as well.
//!     pub fn get_count(&self) -> u32 {
//!         self.count
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (link, _handle): (CounterLink, _) = Actor::spawn(|_ctx| Counter::new());
//!
//!     // Increment the counter
//!     link.increment(5).await.expect("Actor closed");
//!
//!     // Retrieve the value
//!     let val = link.get_count().await.expect("Actor closed");
//!     assert_eq!(val, 5);
//! }
//! ```
//!
//! ## Features
//!
//! Additionally, the crate offers `eframe` feature for integration with [`eframe`].
//! This allows using an eframe app as an actor.
//!
//! ## Limitations
//!
//! Generic actors aren't supported (for now).
//!

/// See crate-level docs for info.
pub use tangled_actors_macros::make_actor;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[cfg(feature = "eframe")]
pub mod eframe;

/// Macro internal
#[doc(hidden)]
pub type ReturnChannelSender<T> = oneshot::Sender<T>;

/// Macro internal
#[doc(hidden)]
pub fn oneshot_channel<T>() -> (ReturnChannelSender<T>, oneshot::Receiver<T>) {
    oneshot::channel()
}

/// Generic "actor link" type. Normally you're gonna use the generated helper struct (nameable with `<actor name>Link` or with trait's associated type [`Actor::Link`])
pub struct ActorLink<T: Actor> {
    sender: mpsc::UnboundedSender<T::Message>,
}

impl<T: Actor> Clone for ActorLink<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

/// Weak version of ActorLink. Does not keep actor it points at alive.
///
/// Can be constructed from any actor link using [`From`] trait.
pub struct WeakLink<T: Actor> {
    sender: mpsc::WeakUnboundedSender<T::Message>,
}

impl<T: Actor> WeakLink<T> {
    pub fn upgrade(&self) -> Option<T::Link> {
        self.sender
            .upgrade()
            .map(|sender| ActorLink { sender }.into())
    }
}

impl<T: Actor> From<&ActorLink<T>> for WeakLink<T> {
    fn from(value: &ActorLink<T>) -> Self {
        Self {
            sender: value.sender.downgrade(),
        }
    }
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

/// Error type.
#[derive(Debug, thiserror::Error)]
#[error("actor is no longer accepting messages")]
pub struct ActorClosed;

impl<T: Actor> ActorLink<T> {
    pub fn send(&self, msg: T::Message) -> Result<(), ActorClosed> {
        self.sender.send(msg).map_err(|_| ActorClosed)
    }
    // TODO: support backpressure
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
