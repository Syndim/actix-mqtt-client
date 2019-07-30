use actix::{Handler, Message, Recipient};
use log::info;
use mqtt::packet::{DisconnectPacket, VariablePacket};

use crate::actors::{ErrorMessage, StopMessage};

use super::{send_packet, VariablePacketMessage};

#[derive(Message)]
pub struct Disconnect;

pub struct DisconnectActor {
    stop_recipient: Recipient<StopMessage>,
    error_recipient: Recipient<ErrorMessage>,
    send_recipient: Recipient<VariablePacketMessage>,
}

impl DisconnectActor {
    pub fn new(
        send_recipient: Recipient<VariablePacketMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
    ) -> Self {
        DisconnectActor {
            send_recipient,
            error_recipient,
            stop_recipient,
        }
    }
}

impl_empty_actor!(DisconnectActor);
impl_stop_handler!(DisconnectActor);

impl Handler<Disconnect> for DisconnectActor {
    type Result = ();
    fn handle(&mut self, _: Disconnect, ctx: &mut Self::Context) -> Self::Result {
        info!("Handle message for DisconnectActor");
        let packet = VariablePacket::DisconnectPacket(DisconnectPacket::new());
        send_packet(
            ctx,
            &self.send_recipient,
            &self.error_recipient,
            &self.stop_recipient,
            packet,
            Disconnect,
        );

        let _ = self.stop_recipient.do_send(StopMessage);
    }
}
