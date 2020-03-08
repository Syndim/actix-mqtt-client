use std::io::ErrorKind;

use actix::{Arbiter, AsyncContext, Handler, Message, Recipient};
use log::info;
use mqtt::packet::{ConnectPacket, VariablePacket};

use crate::actors::actions::status::{PacketStatus, PacketStatusMessages};
use crate::actors::send_error;
use crate::actors::{ErrorMessage, StopMessage};
use crate::consts::{COMMAND_TIMEOUT, PROTOCOL_NAME};

use super::VariablePacketMessage;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub user_name: Option<String>,
    pub password: Option<String>,
    pub keep_alive: Option<u16>,
}

pub struct ConnectActor {
    send_recipient: Recipient<VariablePacketMessage>,
    status_recipient: Recipient<PacketStatusMessages<()>>,
    stop_recipient: Recipient<StopMessage>,
    error_recipient: Recipient<ErrorMessage>,
    client_name: String,
}

impl ConnectActor {
    pub fn new(
        send_recipient: Recipient<VariablePacketMessage>,
        status_recipient: Recipient<PacketStatusMessages<()>>,
        stop_recipient: Recipient<StopMessage>,
        error_recipient: Recipient<ErrorMessage>,
        client_name: String,
    ) -> Self {
        ConnectActor {
            send_recipient,
            status_recipient,
            stop_recipient,
            error_recipient,
            client_name,
        }
    }
}

impl_empty_actor!(ConnectActor);
impl_stop_handler!(ConnectActor);

impl Handler<Connect> for ConnectActor {
    type Result = ();
    fn handle(&mut self, msg: Connect, ctx: &mut Self::Context) -> Self::Result {
        info!("Handle message for ConnectActor");

        // For connect status:
        //      status message with id = 0 indicating the connecing status
        //      status message with id = 1 indicating the connected status
        if let Err(e) = self
            .status_recipient
            .do_send(PacketStatusMessages::SetPacketStatus(
                0,
                PacketStatus {
                    id: 0,
                    retry_count: 0,
                    payload: (),
                },
            ))
        {
            send_error(
                "ConnectActor::status_recipient",
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Failed to set packet status: {}", e),
            );
            let _ = self.stop_recipient.do_send(StopMessage);
            return;
        }

        let mut packet = ConnectPacket::new(PROTOCOL_NAME, &self.client_name);
        packet.set_user_name(msg.user_name);
        packet.set_password(msg.password);
        if let Some(keep_alive) = msg.keep_alive {
            packet.set_keep_alive(keep_alive);
        }

        let connect_variable_packet = VariablePacket::ConnectPacket(packet);
        if let Err(e) = self
            .send_recipient
            .do_send(VariablePacketMessage::new(connect_variable_packet, 0))
        {
            send_error(
                "ConnectActor::send_recipient",
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Failed to send connect packet: {}", e),
            );
            let _ = self.stop_recipient.do_send(StopMessage);
            return;
        }

        let stop_recipient = self.stop_recipient.clone();
        ctx.run_later(COMMAND_TIMEOUT.clone(), |actor, ctx| {
            let addr = ctx.address();
            let error_recipient = actor.error_recipient.clone();
            let status_recipient = actor.status_recipient.clone();
            let status_future = async move {
                let status_result = status_recipient
                    .send(PacketStatusMessages::GetPacketStatus(0))
                    .await;
                match status_result {
                    Ok(status) => {
                        if status.is_some() {
                            send_error(
                                "ConnectActor::status_recipient_ack",
                                &error_recipient,
                                ErrorKind::InvalidData,
                                format!("Doesn't got connect ack from server, exit"),
                            );
                            let _ = stop_recipient.do_send(StopMessage);
                        } else {
                            // Stop the connect actor
                            addr.do_send(StopMessage);
                        }
                    }
                    Err(e) => {
                        send_error(
                            "ConnectActor::status_recipient",
                            &error_recipient,
                            ErrorKind::InvalidData,
                            format!("Failed to check connect ack status: {}", e),
                        );
                        let _ = stop_recipient.do_send(StopMessage);
                    }
                }
            };
            Arbiter::spawn(status_future);
        });
    }
}
