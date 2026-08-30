//! # tangled_actors
//!
//! `tangled_actors` is yet another actor crate with excessive use of macros.
//!
//! This crate simplifies the actor pattern by allowing you to define actor behavior using
//! standard Rust `impl` blocks on actor state struct. A procedural macro `#[make_actor]` generates a private enum containing message types accepted by the actor and
//! a typed "link" struct used to emit these messages, which contains methods with the same name, arguments and visibility.
//!
//! Actors are spawned using `Actor::spawn` trait method.
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
//! #[make_actor]
//! impl Counter {
//!     fn increment(&mut self, amount: u32) {
//!         self.count += amount;
//!     }
//!
//!     fn get_count(&self) -> u32 {
//!         self.count
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let (link, _handle) = Actor::spawn(|_link| Counter { count: 0 });
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
//! ## Key Types
//!
//! - [`Actor`]: The trait that defines the actor's message type and link type.
//! - [`ActorClosed`]: Error returned when the actor task has terminated.

pub use tangled_actors_macros::make_actor;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

/// Macro internal
pub type ReturnChannelSender<T> = oneshot::Sender<T>;

/// Macro internal
pub fn oneshot_channel<T>() -> (ReturnChannelSender<T>, oneshot::Receiver<T>) {
    oneshot::channel()
}

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

#[derive(Debug, thiserror::Error)]
#[error("actor is no longer accepting messages")]
pub struct ActorClosed;

impl<T: Actor> ActorLink<T> {
    pub fn send(&self, msg: T::Message) -> Result<(), ActorClosed> {
        self.sender.send(msg).map_err(|_| ActorClosed)
    }
    // TODO: support backpressure
}

pub trait Actor: Sized + Send + 'static {
    type Message: Send;
    type Link: From<ActorLink<Self>> + Clone + Send;
    /// Spawn an actor task, returning a link object that allows sending messages to this actor.
    fn spawn(
        builder: impl FnOnce(WeakLink<Self>) -> Self + Send + 'static,
    ) -> (Self::Link, JoinHandle<()>) {
        let (sender, mut r) = mpsc::unbounded_channel();
        let link = ActorLink { sender };
        let weak_link = WeakLink::from(&link);
        let task_handle = tokio::spawn({
            async move {
                let mut actor = builder(weak_link);
                // let actor_name = type_name::<Self>();
                while let Some(msg) = r.recv().await {
                    // let _span = span!(Level::TRACE, "actor", actor_name).entered();
                    actor.process_message(msg).await;
                }
            }
        });
        (Self::Link::from(link), task_handle)
    }
    /// Internal, implemented by macros
    fn process_message(
        &mut self,
        message: Self::Message,
    ) -> impl std::future::Future<Output = ()> + Send;
}
