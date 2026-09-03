use std::mem;

use tangled_actors::{Actor, make_actor};

struct TestActor;

#[make_actor]
impl TestActor {
    fn do_things(&self) -> eyre::Result<u32> {
        Ok(42)
    }
}

#[tokio::test]
async fn main() -> eyre::Result<()> {
    let (actor_link, handle) = Actor::spawn(|_link| TestActor);
    assert_eq!(actor_link.do_things().await?, 42);

    mem::drop(actor_link);
    handle.await?;
    Ok(())
}
