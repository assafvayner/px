use std::{future::Future, time::Duration};

use tokio::time::sleep;

pub fn timer<F>(func: F, duration: Duration)
where
    F: Fn() + Send + 'static,
{
    tokio::spawn(async move {
        sleep(duration).await;
        func();
    });
}

pub fn timer_async<T>(future: T, duration: Duration)
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
