use actix::{Handler, Recipient};
use mqtt::packet::{
    ConnackPacket, PingrespPacket, PubackPacket, PubcompPacket, PublishPacket, PubrecPacket,
    PubrelPacket, SubackPacket, UnsubackPacket, VariablePacket,
};

use crate::actors::packets::{PacketMessage, VariablePacketMessage};
use crate::actors::{handle_send_error, ErrorMessage, StopMessage};

pub struct DispatchActor {
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
    connack_recipient: Recipient<PacketMessage<ConnackPacket>>,
    pingresp_recipient: Recipient<PacketMessage<PingrespPacket>>,
    publish_recipient: Recipient<PacketMessage<PublishPacket>>,
    puback_recipient: Recipient<PacketMessage<PubackPacket>>,
    pubrec_recipient: Recipient<PacketMessage<PubrecPacket>>,
    pubrel_recipient: Recipient<PacketMessage<PubrelPacket>>,
    pubcomp_recipient: Recipient<PacketMessage<PubcompPacket>>,
    suback_recipient: Recipient<PacketMessage<SubackPacket>>,
    unsuback_recipient: Recipient<PacketMessage<UnsubackPacket>>,
}

impl DispatchActor {
    pub fn new(
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
        connack_recipient: Recipient<PacketMessage<ConnackPacket>>,
        pingresp_recipient: Recipient<PacketMessage<PingrespPacket>>,
        publish_recipient: Recipient<PacketMessage<PublishPacket>>,
        puback_recipient: Recipient<PacketMessage<PubackPacket>>,
        pubrec_recipient: Recipient<PacketMessage<PubrecPacket>>,
        pubrel_recipient: Recipient<PacketMessage<PubrelPacket>>,
        pubcomp_recipient: Recipient<PacketMessage<PubcompPacket>>,
        suback_recipient: Recipient<PacketMessage<SubackPacket>>,
        unsuback_recipient: Recipient<PacketMessage<UnsubackPacket>>,
    ) -> Self {
        DispatchActor {
            error_recipient,
            stop_recipient,
            connack_recipient,
            pingresp_recipient,
            publish_recipient,
            puback_recipient,
            pubrec_recipient,
            pubcomp_recipient,
            pubrel_recipient,
            suback_recipient,
            unsuback_recipient,
        }
    }
}

impl_empty_actor!(DispatchActor);
impl_stop_handler!(DispatchActor);

impl Handler<VariablePacketMessage> for DispatchActor {
    type Result = ();
    fn handle(&mut self, msg: VariablePacketMessage, _: &mut Self::Context) -> Self::Result {
        macro_rules! send_message {
            ($target:ident, $msg: ident) => {
                if let Err(e) = self.$target.try_send(PacketMessage::new($msg, 0)) {
                    handle_send_error(
                        concat!("DispatchActor:", stringify!($target)),
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                    );
                }
            };
        }

        match msg.packet {
            VariablePacket::ConnackPacket(connack) => {
                send_message!(connack_recipient, connack);
            }
            VariablePacket::PingrespPacket(pingresp) => {
                send_message!(pingresp_recipient, pingresp);
            }
            VariablePacket::PublishPacket(publish) => {
                send_message!(publish_recipient, publish);
            }
            VariablePacket::PubackPacket(puback) => {
                send_message!(puback_recipient, puback);
            }
            VariablePacket::PubrecPacket(pubrec) => {
                send_message!(pubrec_recipient, pubrec);
            }
            VariablePacket::PubrelPacket(pubrel) => {
                send_message!(pubrel_recipient, pubrel);
            }
            VariablePacket::PubcompPacket(pubcomp) => {
                send_message!(pubcomp_recipient, pubcomp);
            }
            VariablePacket::SubackPacket(suback) => {
                send_message!(suback_recipient, suback);
            }
            VariablePacket::UnsubackPacket(unsuback) => {
                send_message!(unsuback_recipient, unsuback);
            }
            _ => (),
        }
    }
}
