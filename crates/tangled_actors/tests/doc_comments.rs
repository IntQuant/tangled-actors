use tangled_actors::{Actor, make_actor};

struct DocActor;

#[make_actor]
impl DocActor {
    /// Increments the count by the given amount.
    fn increment(&mut self, _amount: u32) -> u32 {
        42
    }

    /// Get the current state.
    fn get_state(&self) -> &'static str {
        "ok"
    }
}

#[tokio::test]
async fn doc_commented_functions_work() -> eyre::Result<()> {
    let (link, handle) = Actor::spawn(|_link| DocActor);
    assert_eq!(link.increment(1).await, 42);
    assert_eq!(link.get_state().await, "ok");

    std::mem::drop(link);
    handle.await?;
    Ok(())
}
