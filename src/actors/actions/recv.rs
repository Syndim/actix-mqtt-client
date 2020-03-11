use std::io::ErrorKind;

use actix::{Actor, ActorContext, Context as ActixContext, Handler, Recipient, StreamHandler};
use futures::stream;
use log::{error, trace};
use mqtt::packet::VariablePacket;
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

impl<T: AsyncRead + Unpin + 'static> StreamHandler<VariablePacket> for RecvActor<T> {
    fn handle(&mut self, item: VariablePacket, _ctx: &mut Self::Context) {
        if let Err(e) = self.recipient.try_send(VariablePacketMessage::new(item, 0)) {
            send_error(
                "RecvActor::stream_handler",
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Error when sending packet: {}", e),
            );
        }
    }
}

impl<T: AsyncRead + Unpin + 'static> Actor for RecvActor<T> {
    type Context = ActixContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        trace!("RecvActor started");

        if let Some(reader) = self.stream.take() {
            let error_recipient = self.error_recipient.clone();
            let stop_recipient = self.stop_recipient.clone();
            let packet_stream = stream::unfold(reader, move |mut r| {
                let error_recipient = error_recipient.clone();
                let stop_recipient = stop_recipient.clone();
                async move {
                    let packet_result = VariablePacket::parse(&mut r).await;
                    match packet_result {
                        Ok(packet) => {
                            trace!("Parse packet succeeded");
                            Some((packet, r))
                        }
                        Err(e) => {
                            error!("Failed to parse packet: {}", e);
                            send_error(
                                "RecvActor::parse_packet",
                                &error_recipient,
                                ErrorKind::Interrupted,
                                format!("Error when parsing packet: {}", e),
                            );
                            let _ = stop_recipient.try_send(StopMessage);

                            None
                        }
                    }
                }
            });
            Self::add_stream(packet_stream, ctx);
        } else {
            send_error(
                "RecvActor::stream",
                &self.error_recipient,
                ErrorKind::NotFound,
                "Failed to create packet stream: input stream is None",
            );
            let _ = self.stop_recipient.do_send(StopMessage);
        }
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        trace!("RecvActor stopped");
        let _ = self.stop_recipient.do_send(StopMessage);
    }
}
