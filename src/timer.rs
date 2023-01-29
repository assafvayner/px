use std::{future::Future, time::Duration};

use tokio::time::sleep;

/// executes a given async construct (function, future) after
/// a specified duration
pub fn timer<T>(future: T, duration: Duration)
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    tokio::spawn(async move {
        sleep(duration).await;
        future.await;
        return;
    });
}
