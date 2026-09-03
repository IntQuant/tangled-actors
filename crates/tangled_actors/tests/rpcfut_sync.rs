use std::mem;

use tangled_actors::{Actor, make_actor};
use tokio::task;

struct TestActor {
    val: usize,
}

#[make_actor]
impl TestActor {
    fn get_constant(&self) -> usize {
        42
    }

    fn store_value(&mut self, arg: usize) {
        self.val = arg;
    }

    fn get_value(&mut self) -> usize {
        self.val
    }
}

#[tokio::test]
async fn main() -> eyre::Result<()> {
    let (actor_link, handle) = Actor::spawn(|_ctx| TestActor { val: 0 });

    let thr = task::spawn_blocking({
        // Check that things work in non-async context as well
        let actor_link = actor_link.clone();
        move || {
            let constant = actor_link.get_constant().resolve().unwrap();
            actor_link.store_value(constant).away().unwrap();
        }
    });
    thr.await.unwrap();
    assert_eq!(actor_link.get_value().await.unwrap(), 42);

    mem::drop(actor_link);
    handle.await?;
    Ok(())
}
