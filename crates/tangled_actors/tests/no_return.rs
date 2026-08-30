use std::mem;

use tangled_actors::{Actor, make_actor};

struct TestActor;

#[make_actor]
impl TestActor {
    fn do_things(&self, _arg: u32) {}
}

#[tokio::test]
async fn main() -> eyre::Result<()> {
    let (actor_link, handle) = Actor::spawn(|_link| TestActor);
    actor_link.do_things(32).await?;

    mem::drop(actor_link);
    handle.await?;
    Ok(())
}
