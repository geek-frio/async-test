use std::io::ErrorKind;
use std::io::Read;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::SyncSender;
use std::task::{Poll as TaskPoll, Waker};

use futures::future::BoxFuture;
use futures::Future;
use mio::net::TcpStream;
use mio::Poll;
use mio::{Events, Interest, Token};
use std::sync::Arc;
use std::sync::Mutex;

struct BlankFuture {
    sender: Sender<NonNull<()>>,
    vtable: VTable,
}

struct VTable {
    poll: NonNull<unsafe fn(NonNull<()>, sender: Sender<NonNull<()>>)>,
}

unsafe fn create_blank_future<T>(fut: T) -> (Receiver<NonNull<()>>, BlankFuture)
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    let vtable = VTable {
        poll: NonNull::new(poll::<T> as *mut unsafe fn(NonNull<()>, Sender<NonNull<()>>)).unwrap(),
    };
    (
        receiver,
        BlankFuture {
            sender: sender,
            vtable,
        },
    )
}

impl Future for BlankFuture {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> TaskPoll<Self::Output> {
        todo!()
    }
}

impl Spawner {
    fn spawn<T: Future + Send + 'static>(&self, fut: T) -> Handle<T> {
        // let (send, recv) = std::sync::mpsc::channel();
        // let boxed = fut.boxed();
        // let task = Arc::new(Task {
        //     sender: self.sender.clone(),
        // });
        // self.sender.send(task).unwrap();
        todo!()
    }
}

struct SharedStat {
    waker: Option<Waker>,
    readable: bool,
}

// simple version
fn create_socket_and_poll(addr: &str, id: usize) -> Socket {
    let mut tcp_stream = TcpStream::connect(addr.parse().unwrap()).unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    let shared_state = Arc::new(Mutex::new(SharedStat {
        waker: None,
        readable: false,
    }));
    let sock = Socket {
        receiver: recv,
        shared: shared_state.clone(),
    };
    std::thread::spawn(move || {
        let mut events = Events::with_capacity(1024);
        let mut poller = Poll::new().unwrap();

        poller
            .registry()
            .register(&mut tcp_stream, Token(id), Interest::WRITABLE)
            .unwrap();

        // Confirm connection is ok
        'outer: loop {
            poller.poll(&mut events, None).unwrap();
            for event in events.iter() {
                if event.is_writable() {
                    let peer_addr = tcp_stream.peer_addr();
                    if let Err(e) = peer_addr {
                        match e.kind() {
                            ErrorKind::NotConnected => {
                                continue;
                            }
                            _ => {
                                if e.raw_os_error().is_some() && e.raw_os_error().unwrap() == 127 {
                                    continue;
                                }
                                panic!("When logic come here, it means create connection failed! ");
                            }
                        }
                    } else {
                        println!("create conn success");
                        break 'outer;
                    }
                }
            }
        }

        loop {
            poller.poll(&mut events, None).unwrap();
            for event in events.iter() {
                if event.is_readable() && event.token() == Token(id) {
                    let mut buf = Vec::new();
                    tcp_stream.read_to_end(&mut buf).unwrap();
                    let mut guard = shared_state.lock().unwrap();
                    guard.readable = true;
                    send.send(buf).unwrap();
                    if guard.waker.is_some() {
                        guard.waker.as_ref().unwrap().wake_by_ref();
                    }
                }
            }
        }
    });
    sock
}

struct Socket {
    shared: Arc<Mutex<SharedStat>>,
    receiver: Receiver<Vec<u8>>,
}

impl Socket {
    fn new(addr: &str, id: usize) -> Socket {
        create_socket_and_poll(addr, id)
    }

    fn is_readable(&self) -> bool {
        let stat = self.shared.lock().unwrap();
        stat.readable
    }
}

impl Future for Socket {
    type Output = Vec<u8>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.is_readable() {
            let buf = self.receiver.recv().unwrap();
            return std::task::Poll::Ready(buf);
        } else {
            let waker = cx.waker().clone();
            let mut guard = self.shared.lock().unwrap();
            guard.readable = false;
            guard.waker = Some(waker);
            return TaskPoll::Pending;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mio::{net::TcpListener, Poll};

    #[test]
    fn test_poll_before_regist() -> Result<(), anyhow::Error> {
        // let mut listener = TcpListener::bind("127.0.0.1:34245".parse()?)?;
        // let mut poll = Poll::new()?;

        // register action must be before poll operation, poll operation need a "&mut self"

        todo!()
    }
}
