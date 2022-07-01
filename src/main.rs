pub(crate) mod run_vtable;
use std::{
    sync::{Arc, Mutex},
    task::Waker,
    thread::sleep,
    time::Duration,
};

use futures::Future;
use run_trick::*;
mod run_trick;

struct TimedFuture {
    shared: Arc<Mutex<SharedStat>>,
}

impl TimedFuture {
    fn new(duration: Duration) -> TimedFuture {
        let shared = Arc::new(Mutex::new(SharedStat {
            complete: false,
            waker: None,
        }));
        let fut = TimedFuture {
            shared: shared.clone(),
        };
        std::thread::spawn(move || {
            println!("started");
            let mut guard = shared.lock().unwrap();
            guard.complete = true;
            sleep(duration);
            println!("sleeped!");
            if let Some(waker) = guard.waker.as_ref() {
                waker.wake_by_ref();
                println!("wake has called!");
            }
        });
        fut
    }
}

struct SharedStat {
    complete: bool,
    waker: Option<Waker>,
}

impl Future for TimedFuture {
    type Output = u64;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut shared = self.shared.lock().unwrap();
        if shared.complete {
            return std::task::Poll::Ready(1);
        } else {
            shared.waker = Some(cx.waker().clone());
            return std::task::Poll::Pending;
        }
    }
}
fn main() {
    let (spawner, executor) = create_runtime();
    executor.run();
    let fut = TimedFuture::new(Duration::from_secs(3));
    let mut handle = spawner.spawn(fut);
    println!("{:?}", handle.get_data());
}
