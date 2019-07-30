use std::io::ErrorKind;

use actix::{Arbiter, AsyncContext, Handler, Message, Recipient};
use futures::Future;
use log::info;
use mqtt::packet::{ConnectPacket, VariablePacket};

use crate::actors::actions::status::{PacketStatus, PacketStatusMessages};
use crate::actors::{force_stop_system, send_error};
use crate::actors::{ErrorMessage, StopMessage};
use crate::consts::{COMMAND_TIMEOUT, PROTOCOL_NAME};
use crate::errors;

use super::VariablePacketMessage;

#[derive(Message)]
pub struct Connect {
    pub user_name: Option<String>,
    pub password: Option<String>,
    pub keep_alive: Option<u16>,
}

pub struct ConnectActor {
    send_recipient: Recipient<VariablePacketMessage>,
    status_recipient: Recipient<PacketStatusMessages<()>>,
    error_recipient: Recipient<ErrorMessage>,
    client_name: String,
}

impl ConnectActor {
    pub fn new(
        send_recipient: Recipient<VariablePacketMessage>,
        status_recipient: Recipient<PacketStatusMessages<()>>,
        error_recipient: Recipient<ErrorMessage>,
        client_name: String,
    ) -> Self {
        ConnectActor {
            send_recipient,
            status_recipient,
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
        if let Err(e) = self
            .status_recipient
            .do_send(PacketStatusMessages::SetPacketStatus(
                0,
                PacketStatus {
                    id: 0,
                    retry_time: 0,
                    payload: (),
                },
            ))
        {
            send_error(
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Failed to set packet status: {}", e),
            );
            force_stop_system(errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
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
                &self.error_recipient,
                ErrorKind::Interrupted,
                format!("Failed to send connect packet: {}", e),
            );
            force_stop_system(errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
            return;
        }

        ctx.run_later(COMMAND_TIMEOUT.clone(), |actor, ctx| {
            let addr = ctx.address();
            let error_recipient = actor.error_recipient.clone();
            let error_recipient_clone = actor.error_recipient.clone();
            Arbiter::spawn(
                actor
                    .status_recipient
                    .send(PacketStatusMessages::GetPacketStatus(0))
                    .map(move |status| {
                        if status.is_some() {
                            send_error(
                                &error_recipient,
                                ErrorKind::InvalidData,
                                format!("Doesn't got connect ack from server, exit"),
                            );
                            force_stop_system(
                                errors::ERROR_CODE_FAILED_TO_RECV_RESPONSE_FROM_SERVER,
                            );
                        } else {
                            // Stop the connect actor
                            addr.do_send(StopMessage);
                        }
                    })
                    .map_err(move |e| {
                        send_error(
                            &error_recipient_clone,
                            ErrorKind::InvalidData,
                            format!("Failed to check connect ack status: {}", e),
                        );
                        force_stop_system(errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
                    }),
            );
        });
    }
}
