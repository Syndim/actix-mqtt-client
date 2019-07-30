use std::io::{ErrorKind, Write};

use actix::{Actor, ActorContext, Context, Handler, Recipient};
use log::info;
use mqtt::encodable::Encodable;

use crate::actors::packets::VariablePacketMessage;
use crate::actors::{send_error, ErrorMessage, StopMessage};

pub struct SendActor<T: Write> {
    stream: T,
    error_recipient: Recipient<ErrorMessage>,
}

impl<T: Write> SendActor<T> {
    pub fn new(stream: T, error_recipient: Recipient<ErrorMessage>) -> Self {
        SendActor {
            stream,
            error_recipient,
        }
    }
}

impl<T: Write + 'static> Actor for SendActor<T> {
    type Context = Context<Self>;
    fn started(&mut self, _: &mut Self::Context) {
        info!("SendActor started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("SendActor stopped");
    }
}

impl<T: Write + 'static> actix::Handler<StopMessage> for SendActor<T> {
    type Result = ();

    fn handle(&mut self, _: StopMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

impl<T: Write + 'static> Handler<VariablePacketMessage> for SendActor<T> {
    type Result = ();
    fn handle(&mut self, msg: VariablePacketMessage, _: &mut Self::Context) -> Self::Result {
        // TODO: Should we use async write here?
        if let Err(e) = msg.packet.encode(&mut self.stream) {
            send_error(
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Failed to send message, error: {}", e),
            );
        }
    }
}
