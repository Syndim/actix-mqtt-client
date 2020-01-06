use std::io::ErrorKind;

use actix::{Actor, ActorContext, Context as ActixContext, Handler, Recipient, StreamHandler};
use futures::stream;
use log::info;
use mqtt::packet::{VariablePacket, VariablePacketError};
use tokio::io::AsyncRead;

use crate::actors::packets::VariablePacketMessage;
use crate::actors::{send_error, ErrorMessage, StopMessage};

pub struct RecvActor<T: AsyncRead + Unpin> {
    stream: Option<T>,
    recipient: Recipient<VariablePacketMessage>,
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
}

impl<T: AsyncRead + Unpin> RecvActor<T> {
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

impl<T: AsyncRead + Unpin + 'static> Handler<StopMessage> for RecvActor<T> {
    type Result = ();

    fn handle(&mut self, _: StopMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

impl<T: AsyncRead + Unpin + 'static> StreamHandler<Result<VariablePacket, VariablePacketError>>
    for RecvActor<T>
{
    fn handle(
        &mut self,
        item: Result<VariablePacket, VariablePacketError>,
        _ctx: &mut Self::Context,
    ) {
        info!("Got packet");
        match item {
            Ok(packet) => {
                if let Err(e) = self
                    .recipient
                    .try_send(VariablePacketMessage::new(packet, 0))
                {
                    send_error(
                        &self.error_recipient,
                        ErrorKind::Interrupted,
                        format!("Error when sending packet: {}", e),
                    );
                }
            }
            Err(e) => {
                send_error(
                    &self.error_recipient,
                    ErrorKind::Interrupted,
                    format!("Error when parsing packet: {}", e),
                );
            }
        }
    }
}

impl<T: AsyncRead + Unpin + 'static> Actor for RecvActor<T> {
    type Context = ActixContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("RecvActor started");

        if let Some(reader) = self.stream.take() {
            let packet_stream = stream::unfold(reader, |mut r| {
                async {
                    let packet = VariablePacket::parse(&mut r).await;
                    Some((packet, r))
                }
            });
            Self::add_stream(packet_stream, ctx);
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
