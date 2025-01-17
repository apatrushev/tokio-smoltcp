use crate::device::{self, FutureDevice};
use crate::socket_alloctor::SocketAlloctor;
use crate::BufferSize;
use smoltcp::{
    iface::EthernetInterface,
    time::{Duration, Instant},
};
use std::{future::Future, sync::Arc};
use tokio::{pin, select, sync::Notify, time::sleep};

pub struct Reactor {
    socket_alloctor: Arc<SocketAlloctor>,
    notify: Arc<Notify>,
}

async fn run<S: device::Interface + 'static>(
    mut interf: EthernetInterface<'static, FutureDevice<S>>,
    sockets: Arc<SocketAlloctor>,
    notify: Arc<Notify>,
    stopper: Arc<Notify>,
) {
    let default_timeout = Duration::from_secs(60);
    let timer = sleep(default_timeout.into());
    pin!(timer);

    loop {
        interf
            .device_mut()
            .send_queue()
            .await
            .expect("Failed to send queue");

        if interf.device_mut().need_wait() {
            let start = Instant::now();
            let deadline = {
                interf
                    .poll_delay(&*sockets.lock(), start)
                    .unwrap_or(default_timeout)
            };

            timer
                .as_mut()
                .reset(tokio::time::Instant::now() + deadline.into());
            select! {
                _ = &mut timer => {},
                _ = interf.device_mut().wait() => {}
                _ = notify.notified() => {}
                _ = stopper.notified() => break,
            };
        }

        while !matches!(
            interf.poll(&mut sockets.lock(), Instant::now()),
            Ok(_) | Err(smoltcp::Error::Exhausted)
        ) {}
    }
}

impl Reactor {
    pub fn new<S: device::Interface + 'static>(
        interf: EthernetInterface<'static, FutureDevice<S>>,
        buffer_size: BufferSize,
        stopper: Arc<Notify>,
    ) -> (Self, impl Future<Output = ()> + Send) {
        let socket_alloctor = Arc::new(SocketAlloctor::new(buffer_size));
        let notify = Arc::new(Notify::new());
        let fut = run(interf, socket_alloctor.clone(), notify.clone(), stopper);

        (
            Reactor {
                socket_alloctor,
                notify,
            },
            fut,
        )
    }
    pub fn socket_alloctor(&self) -> &Arc<SocketAlloctor> {
        &self.socket_alloctor
    }
    pub fn notify(&self) {
        self.notify.notify_waiters();
    }
}
