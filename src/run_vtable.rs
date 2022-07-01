use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicPtr, Ordering},
        mpsc::{Receiver, SyncSender},
        Arc,
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
                unsafe {
                    (task.vtab.poll)(task.raw.load(Ordering::Relaxed), task.clone());
                }
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
        let handle = Handle {
            rx: recv,
            marker: PhantomData::<T>,
        };
        let raw = AtomicPtr::new(Box::into_raw(Box::new(fut.boxed())) as *mut ());
        let vtab = VTable { poll: poll::<T> };

        let task = Task {
            raw,
            vtab,
            task_sender: self.sender.clone(),
            res_sender: send,
        };
        let _ = self.sender.send(Arc::new(task));
        handle
    }
}

pub(crate) struct Task {
    raw: AtomicPtr<()>,
    vtab: VTable,
    task_sender: SyncSender<Arc<Task>>,
    res_sender: SyncSender<AtomicPtr<()>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let r = arc_self.task_sender.send(arc_self.clone());
        println!("wake send result is:{:?}", r);
    }
}

struct VTable {
    poll: unsafe fn(*mut (), task: Arc<Task>),
}

unsafe fn poll<T>(ptr: *mut (), task: Arc<Task>)
where
    T: Send + 'static,
{
    let mut future = *Box::from_raw(ptr as *mut BoxFuture<T>);
    let wf = waker_ref(&task);
    let mut ctx = Context::from_waker(&*wf);
    let res = future.as_mut().poll(&mut ctx);
    if let std::task::Poll::Ready(t) = res {
        let _ = task
            .res_sender
            .send(AtomicPtr::new(Box::into_raw(Box::new(t)) as *mut ()));
    } else {
        let ptr = Box::into_raw(Box::new(future));
        task.raw.store(ptr as *mut (), Ordering::Relaxed);
    }
}

pub struct Handle<T> {
    rx: Receiver<AtomicPtr<()>>,
    marker: PhantomData<T>,
}

impl<T> Handle<T> {
    pub fn get_data(&mut self) -> Result<T, anyhow::Error> {
        unsafe {
            let result = self.rx.recv();
            result
                .map(|ptr| *Box::from_raw(ptr.load(Ordering::Relaxed) as *mut T))
                .map_err(|_e| anyhow::Error::msg("sss"))
        }
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
