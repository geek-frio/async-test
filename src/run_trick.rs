use std::{
    sync::{
        mpsc::{Receiver, SyncSender},
        Arc, Mutex,
    },
    task::Context,
};

use futures::{
    future::BoxFuture,
    task::{waker_ref, ArcWake},
    Future, FutureExt,
};

pub struct Executor {
    receiver: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn run(self) {
        let receiver = self.receiver;
        std::thread::spawn(move || {
            while let Ok(task) = receiver.recv() {
                println!("Has found a new task!");
                let mut fut = task.fut.lock().unwrap();
                let task_c = task.clone();

                let w = &*waker_ref(&task_c);
                let mut ctx = Context::from_waker(w);
                let _ = fut.as_mut().poll(&mut ctx);
            }
        });
    }
}

pub struct Spawner {
    sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn<T>(&self, fut: impl Future<Output = T> + Send + 'static) -> Handle<T>
    where
        T: Send + 'static,
    {
        let (send, recv) = std::sync::mpsc::sync_channel(1024);
        let handle = Handle { rx: recv };

        let blank_fut = async move {
            let r = fut.await;
            let _ = send.send(r);
        };
        let task = Task {
            fut: Arc::new(Mutex::new(blank_fut.boxed())),
            task_sender: self.sender.clone(),
        };
        let _ = self.sender.send(Arc::new(task));
        handle
    }
}

pub(crate) struct Task {
    fut: Arc<Mutex<BoxFuture<'static, ()>>>,
    task_sender: SyncSender<Arc<Task>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let r = arc_self.task_sender.send(arc_self.clone());
        println!("wake send result is:{:?}", r);
    }
}

pub struct Handle<T> {
    rx: Receiver<T>,
}

impl<T> Handle<T> {
    pub fn get_data(&mut self) -> Result<T, anyhow::Error> {
        self.rx.recv().map_err(|e| e.into())
    }
}

pub fn create_runtime() -> (Spawner, Executor) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1024);
    let executor = Executor { receiver };
    let spawner = Spawner { sender };
    (spawner, executor)
}

#[cfg(test)]
mod test {}
