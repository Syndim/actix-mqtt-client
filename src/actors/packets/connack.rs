use std::io::ErrorKind;

use actix::{ActorContext, Handler, Recipient};
use log::info;
use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::ConnackPacket;

use crate::actors::actions::status::{PacketStatus, PacketStatusMessages};
use crate::actors::{send_error, ErrorMessage, StopMessage};

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
                "ConnackActor::status_recipient",
                &self.error_recipient,
                ErrorKind::NotConnected,
                format!("Failed to mark status for connack packet: {}", e),
            );
            let _ = self.stop_recipient.do_send(StopMessage);
        } else {
            let return_code = msg.packet.connect_return_code();
            if return_code == ConnectReturnCode::ConnectionAccepted {
                // For connect status:
                //      status message with id = 0 indicating the connecing status
                //      status message with id = 1 indicating the connected status
                let _ = self
                    .status_recipient
                    .do_send(PacketStatusMessages::SetPacketStatus(
                        1,
                        PacketStatus {
                            id: 1,
                            retry_count: 0,
                            payload: (),
                        },
                    ));
                let _ = self.connect_stop_recipient.do_send(StopMessage);
                ctx.stop();
            } else {
                send_error(
                    "ConnackActor::connect",
                    &self.error_recipient,
                    ErrorKind::NotConnected,
                    format!(
                        "Failed to connect to server, server returns: {:?}",
                        return_code
                    ),
                );

                let _ = self.stop_recipient.do_send(StopMessage);
            }
        }
    }
}
