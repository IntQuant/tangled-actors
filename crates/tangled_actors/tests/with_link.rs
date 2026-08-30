use std::{mem, time::Duration};

use tangled_actors::{Actor, WeakLink, make_actor};

struct TestActor {
    link: WeakLink<Self>,
    ret: u32,
}

#[make_actor]
impl TestActor {
    fn do_things(&self, _arg: u32) -> eyre::Result<u32> {
        let link = self.link.upgrade().unwrap();
        tokio::spawn(async move {
            link.update_ret().await.unwrap();
        });
        Ok(42)
    }
    fn update_ret(&mut self) {
        self.ret = 42;
    }
    fn check_ret(&self) {
        assert_eq!(self.ret, 42);
    }
}

#[tokio::test]
async fn main() -> eyre::Result<()> {
    let (actor_link, handle) = Actor::spawn(|link| TestActor { link, ret: 0 });
    assert_eq!(actor_link.do_things(32).await??, 42);
    tokio::time::sleep(Duration::from_millis(10)).await;
    let _ = actor_link.check_ret().await;

    mem::drop(actor_link);
    handle.await?;
    Ok(())
}
