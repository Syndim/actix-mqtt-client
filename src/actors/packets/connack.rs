use std::io::ErrorKind;

use actix::{ActorContext, Handler, Recipient};
use log::info;
use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::ConnackPacket;

use crate::actors::actions::status::PacketStatusMessages;
use crate::actors::{send_error, stop_system, ErrorMessage, StopMessage};
use crate::errors;

use super::PacketMessage;

pub struct ConnackActor {
    status_recipient: Recipient<PacketStatusMessages<()>>,
    error_recipient: Recipient<ErrorMessage>,
    connect_stop_recipient: Recipient<StopMessage>,
    stop_recipient: Recipient<StopMessage>,
}

impl ConnackActor {
    pub fn new(
        status_recipient: Recipient<PacketStatusMessages<()>>,
        error_recipient: Recipient<ErrorMessage>,
        connect_stop_recipient: Recipient<StopMessage>,
        stop_recipient: Recipient<StopMessage>,
    ) -> Self {
        ConnackActor {
            status_recipient,
            error_recipient,
            connect_stop_recipient,
            stop_recipient,
        }
    }
}

impl_empty_actor!(ConnackActor);
impl_stop_handler!(ConnackActor);

impl Handler<PacketMessage<ConnackPacket>> for ConnackActor {
    type Result = ();
    fn handle(
        &mut self,
        msg: PacketMessage<ConnackPacket>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("Handle message for ConnackActor");
        if let Err(e) = self
            .status_recipient
            .try_send(PacketStatusMessages::RemovePacketStatus(0))
        {
            send_error(
                &self.error_recipient,
                ErrorKind::NotConnected,
                format!("Failed to mark status for connack packet: {}", e),
            );
            stop_system(&self.stop_recipient, errors::ERROR_CODE_SERVER_RETURNS_FAIL)
        } else {
            let return_code = msg.packet.connect_return_code();
            if return_code == ConnectReturnCode::ConnectionAccepted {
                ctx.stop();
                let _ = self.connect_stop_recipient.do_send(StopMessage);
            } else {
                send_error(
                    &self.error_recipient,
                    ErrorKind::NotConnected,
                    format!(
                        "Failed to connect to server, server returns: {:?}",
                        return_code
                    ),
                );
                stop_system(
                    &self.stop_recipient,
                    errors::ERROR_CODE_SERVER_REFUSE_TO_CONNECT,
                );
            }
        }
    }
}
