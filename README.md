# Yet another actor crate with excessive use of macros.

The core of this crate is the [`make_actor`] macro, which is used on an impl block to generate an [`Actor`] trait impl, a message enum,
and an "actor link" struct with rpc-style methods.
Notably, every generated method inherits visibility of the original function.

Actors are spawned using [`Actor::spawn`] trait method.

## Example

```rust
use tangled_actors::{Actor, make_actor};

struct Counter {
    count: u32,
}

// Separate impl block can be used for non-actor methods.
impl Counter {
    fn new() -> Self {
        Self { count: 0 }
    }
}

// Every function defined here will be turned into actor methods.
#[make_actor]
impl Counter {
    fn increment(&mut self, amount: u32) {
        self.count += amount;
    }

    // `get_count` on generated CounterLink type will be public as well.
    pub fn get_count(&self) -> u32 {
        self.count
    }
}

#[tokio::main]
async fn main() {
    let (link, _handle): (CounterLink, _) = Actor::spawn(|_ctx| Counter::new());

    // Increment the counter
    link.increment(5).await.expect("Actor closed");

    // Retrieve the value
    let val = link.get_count().await.expect("Actor closed");
    assert_eq!(val, 5);
}
```

## Features

Additionally, the crate offers `eframe` feature for integration with [`eframe`].
This allows using an eframe app as an actor.

## Limitations

Generic actors aren't supported (for now).
