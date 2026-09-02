use tokio::sync::mpsc;

use crate::{Actor, ActorClosed};

/// Generic "actor link" type. Normally you're gonna use the generated helper struct (nameable with `<actor name>Link` or with trait's associated type [`Actor::Link`])
pub struct ActorLink<T: Actor> {
    pub(crate) sender: mpsc::UnboundedSender<T::Message>,
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
    pub(crate) sender: mpsc::WeakUnboundedSender<T::Message>,
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

impl<T: Actor> ActorLink<T> {
    pub fn send(&self, msg: T::Message) -> Result<(), ActorClosed> {
        self.sender.send(msg).map_err(|_| ActorClosed)
    }
    // TODO: support backpressure
}
