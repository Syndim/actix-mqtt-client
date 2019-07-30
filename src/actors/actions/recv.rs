use std::boxed::Box;
use std::io::{Error as IoError, ErrorKind};

use actix::{Actor, ActorContext, Context, Handler, Recipient, StreamHandler};
use futures::{Async, Future, Poll, Stream};
use log::{error, info};
use mqtt::packet::{VariablePacket, VariablePacketError};
use tokio::io::AsyncRead;
use tokio::prelude::task;

use crate::actors::packets::VariablePacketMessage;
use crate::actors::{send_error, ErrorMessage, StopMessage};

struct PacketStream<TReader: AsyncRead + 'static> {
    reader: Option<TReader>,
    future: Option<Box<dyn Future<Item = (TReader, VariablePacket), Error = VariablePacketError>>>,
    error_recipient: Recipient<ErrorMessage>,
}

impl<TReader: AsyncRead + 'static> PacketStream<TReader> {
    pub fn new(reader: TReader, error_recipient: Recipient<ErrorMessage>) -> Self {
        PacketStream {
            reader: Some(reader),
            future: None,
            error_recipient,
        }
    }
}

impl<TReader: AsyncRead + 'static> Stream for PacketStream<TReader> {
    type Item = VariablePacket;
    type Error = VariablePacketError;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        info!("Poll recv stream");
        if let Some(ref mut future) = self.future {
            match future.poll() {
                Ok(Async::Ready((reader, packet))) => {
                    info!("Recv packet ready");
                    self.future = Some(Box::new(VariablePacket::parse(reader)));
                    Ok(Async::Ready(Some(packet)))
                }
                Ok(Async::NotReady) => {
                    info!("Recv packet not ready");
                    Ok(Async::NotReady)
                }
                Err(e) => {
                    send_error(
                        &self.error_recipient,
                        ErrorKind::Interrupted,
                        format!("Recv packet error: {}", e),
                    );
                    Err(e)
                }
            }
        } else {
            let reader_option = self.reader.take();
            match reader_option {
                Some(reader) => {
                    info!("Create reader");
                    self.future = Some(Box::new(VariablePacket::parse(reader)));
                    task::current().notify();
                    Ok(Async::NotReady)
                }
                None => {
                    error!("Reader is none");
                    Err(VariablePacketError::IoError(IoError::new(
                        ErrorKind::InvalidData,
                        "Reader is none",
                    )))
                }
            }
        }
    }
}

pub struct RecvActor<T: AsyncRead> {
    stream: Option<T>,
    recipient: Recipient<VariablePacketMessage>,
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
}

impl<T: AsyncRead> RecvActor<T> {
    pub fn new(
        stream: T,
        recipient: Recipient<VariablePacketMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
    ) -> Self {
        RecvActor {
            stream: Some(stream),
            recipient,
            error_recipient,
            stop_recipient,
        }
    }
}

impl<T: AsyncRead + 'static> Handler<StopMessage> for RecvActor<T> {
    type Result = ();

    fn handle(&mut self, _: StopMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

impl<T: AsyncRead + 'static> StreamHandler<VariablePacket, VariablePacketError> for RecvActor<T> {
    fn handle(&mut self, item: VariablePacket, _ctx: &mut Self::Context) {
        if let Err(e) = self.recipient.try_send(VariablePacketMessage::new(item, 0)) {
            send_error(
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Error when sending packet: {}", e),
            );
        }
    }
}

impl<T: AsyncRead + 'static> Actor for RecvActor<T> {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("RecvActor started");

        if let Some(reader) = self.stream.take() {
            let stream = PacketStream::new(reader, self.error_recipient.clone());
            Self::add_stream(stream, ctx);
        } else {
            send_error(
                &self.error_recipient,
                ErrorKind::NotFound,
                "Failed to create packet stream: input stream is None",
            );
            let _ = self.stop_recipient.do_send(StopMessage);
        }
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("RecvActor stopped");
        let _ = self.stop_recipient.do_send(StopMessage);
    }
}
