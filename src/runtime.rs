use std::{
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr::NonNull,
    sync::{
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
    pub fn run(&mut self, sender: SyncSender<Arc<Task>>) {
        while let Ok(task) = self.receiver.recv() {
            unsafe {
                (task.vtab.poll)(task.raw.raw, task.clone());
            }
        }
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
        let raw = NonNullWrapper::new(
            NonNull::new(Box::into_raw(Box::new(fut.boxed())) as *mut ()).unwrap(),
        );
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

struct NonNullWrapper {
    raw: NonNull<()>,
}

impl NonNullWrapper {
    fn new(ptr: NonNull<()>) -> NonNullWrapper {
        NonNullWrapper { raw: ptr }
    }
}

impl Deref for NonNullWrapper {
    type Target = NonNull<()>;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

unsafe impl Send for NonNullWrapper {}
unsafe impl Sync for NonNullWrapper {}

pub(crate) struct Task {
    raw: NonNullWrapper,
    vtab: VTable,
    task_sender: SyncSender<Arc<Task>>,
    res_sender: SyncSender<NonNullWrapper>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let _ = arc_self.task_sender.send(arc_self.clone());
    }
}

struct VTable {
    poll: unsafe fn(NonNull<()>, task: Arc<Task>),
}

unsafe fn poll<T>(ptr: NonNull<()>, task: Arc<Task>)
where
    T: Send + 'static,
{
    let mut future = *Box::from_raw(ptr.as_ptr() as *mut BoxFuture<T>);
    let wf = waker_ref(&task);
    let mut ctx = Context::from_waker(&*wf);
    let res = future.as_mut().poll(&mut ctx);
    if let std::task::Poll::Ready(t) = res {
        let _ = task.res_sender.send(NonNullWrapper::new(
            NonNull::new(Box::into_raw(Box::new(t)) as *mut ()).unwrap(),
        ));
    }
    let _ = ManuallyDrop::new(future);
}

pub struct Handle<T> {
    rx: Receiver<NonNullWrapper>,
    marker: PhantomData<T>,
}

impl<T> Handle<T> {
    pub fn get_data(&mut self) -> Result<T, anyhow::Error> {
        unsafe {
            let result = self.rx.recv();
            result
                .map(|ptr| *Box::from_raw(ptr.as_ptr() as *mut T))
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
