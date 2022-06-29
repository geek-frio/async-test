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
};

struct Executor {
    receiver: Receiver<Arc<Task>>,
}

impl Executor {
    fn run(&mut self, sender: SyncSender<Arc<Task>>) {
        while let Ok(task) = self.receiver.recv() {
            unsafe {
                (task.vtab.poll)(task.raw.raw, &sender, task.clone());
            }
        }
    }
}

struct Spawner {
    sender: SyncSender<Arc<Task>>,
}

struct NonNullWrapper {
    raw: NonNull<()>,
}

impl Deref for NonNullWrapper {
    type Target = NonNull<()>;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

unsafe impl Send for NonNullWrapper {}
unsafe impl Sync for NonNullWrapper {}

struct Task {
    raw: NonNullWrapper,
    vtab: VTable,
    sender: SyncSender<Arc<Task>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let _ = arc_self.sender.send(arc_self.clone());
    }
}

struct VTable {
    poll: unsafe fn(NonNull<()>, sender: &SyncSender<NonNull<()>>, task: Arc<Task>),
}

unsafe fn poll<T>(ptr: NonNull<()>, sender: &Sender<NonNull<()>>, task: Arc<Task>)
where
    T: Send + 'static,
{
    let mut future = *Box::from_raw(ptr.as_ptr() as *mut BoxFuture<T>);
    let wf = waker_ref(&task);
    let mut ctx = Context::from_waker(&*wf);
    future.as_mut().poll(&mut ctx);
    ManuallyDrop::new(future);
}

struct Handle<T> {
    rx: Receiver<NonNull<()>>,
    marker: PhantomData<T>,
}

impl<T> Handle<T> {
    fn get_data(&mut self) -> Result<T, anyhow::Error> {
        unsafe {
            let result = self.rx.recv();
            result
                .map(|ptr| *Box::from_raw(ptr.as_ptr() as *mut T))
                .map_err(|_e| anyhow::Error::msg("sss"))
        }
    }
}

fn create_runtime() -> (Spawner, Executor) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1024);
    let executor = Executor { receiver };
    let spawner = Spawner { sender };
    (spawner, executor)
}
