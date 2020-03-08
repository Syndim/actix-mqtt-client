use std::io::{Error, ErrorKind};

use actix::io::{WriteHandler, Writer};
use actix::{Actor, ActorContext, Context, Handler, Recipient, Running};
use log::{error, info};
use mqtt::encodable::Encodable;
use tokio::io::AsyncWrite;

use crate::actors::packets::VariablePacketMessage;
use crate::actors::{send_error, ErrorMessage, StopMessage};

pub struct SendActor<T: AsyncWrite> {
    stream: Option<T>,
    writer: Option<Writer<T, Error>>,
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
}

impl<T: AsyncWrite + Unpin> SendActor<T> {
    pub fn new(
        stream: T,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
    ) -> Self {
        SendActor {
            stream: Some(stream),
            writer: None,
            error_recipient,
            stop_recipient,
        }
    }
}

impl<T: AsyncWrite + Unpin + 'static> WriteHandler<Error> for SendActor<T> {
    fn error(&mut self, err: Error, _ctx: &mut Self::Context) -> Running {
        error!("Error in write handler, {:?}", err);
        send_error(
            "SendActor::error",
            &self.error_recipient,
            ErrorKind::Interrupted,
            format!("{:?}", err),
        );
        let _ = self.stop_recipient.do_send(StopMessage);
        Running::Stop
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        info!("Writer finished");
        ctx.stop()
    }
}

impl<T: AsyncWrite + Unpin + 'static> Actor for SendActor<T> {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        let stream = self.stream.take().unwrap();
        self.writer = Some(Writer::new(stream, ctx));
        info!("SendActor started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("SendActor stopped");
    }
}

impl<T: AsyncWrite + Unpin + 'static> actix::Handler<StopMessage> for SendActor<T> {
    type Result = ();

    fn handle(&mut self, _: StopMessage, ctx: &mut Self::Context) -> Self::Result {
        info!("Got stop message");
        ctx.stop();
    }
}

impl<T: AsyncWrite + Unpin + 'static> Handler<VariablePacketMessage> for SendActor<T> {
    type Result = ();
    fn handle(&mut self, msg: VariablePacketMessage, _: &mut Self::Context) -> Self::Result {
        if self.writer.is_none() {
            error!("Writer is none");
            send_error(
                "SendActor::writer",
                &self.error_recipient,
                ErrorKind::NotFound,
                "Writer is none",
            );

            return;
        }

        let mut buf = Vec::new();
        if let Err(e) = msg.packet.encode(&mut buf) {
            error!("Failed to encode message, error {}", e);
            send_error(
                "SendActor::encode",
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Failed to send message, error: {}", e),
            );
        }

        let writer = self.writer.as_mut().unwrap();
        writer.write(&*buf);
    }
}
