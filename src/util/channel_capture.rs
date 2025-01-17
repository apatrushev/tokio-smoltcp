use futures::{Sink, Stream};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::{channel, error::SendError, Receiver, Sender};
use tokio_util::sync::PollSender;

pub struct ChannelCapture {
    recv: Receiver<Vec<u8>>,
    send: PollSender<Vec<u8>>,
}

impl ChannelCapture {
    pub fn new<R, S>(recv: R, send: S) -> Self
    where
        S: FnOnce(Receiver<Vec<u8>>) + Send + 'static,
        R: FnOnce(Sender<Vec<u8>>) + Send + 'static,
    {
        let (tx1, rx1) = channel(1000);
        let (tx2, rx2) = channel(1000);
        std::thread::spawn(move || {
            recv(tx2);
            eprintln!("Recv thread exited")
        });
        std::thread::spawn(move || {
            send(rx1);
            eprintln!("Send thread exited")
        });
        ChannelCapture {
            send: PollSender::new(tx1),
            recv: rx2,
        }
    }
}

impl Stream for ChannelCapture {
    type Item = Vec<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.recv.poll_recv(cx)
    }
}

fn map_err(e: SendError<Vec<u8>>) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

impl Sink<Vec<u8>> for ChannelCapture {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.send.poll_send_done(cx).map_err(map_err)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        self.send.start_send(item).map_err(map_err)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.send.poll_send_done(cx).map_err(map_err)
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
