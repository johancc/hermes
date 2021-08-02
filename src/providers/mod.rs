pub mod google;
use async_trait::async_trait;

#[async_trait]
pub trait Provider {
    /// Returns the total amount of steps that the user has taken between now and midnight.
    /// "midnight" is relative to the current time.
    async fn daily_steps(&self) -> anyhow::Result<i32>;
}
