use mqtt::packet::{PubcompPacket, PubrelPacket};

use crate::actors::actions::status::PacketStatusMessages;
use crate::actors::packets::{PacketMessage, PublishPacketStatus};

fn get_retry_count_from_message(msg: &PacketMessage<PubrelPacket>) -> u16 {
    msg.retry_count
}

fn create_retry_message_from_message(
    msg: PacketMessage<PubrelPacket>,
) -> PacketMessage<PubrelPacket> {
    let mut retry_msg = msg;
    retry_msg.retry_count += 1;
    retry_msg
}

fn create_packet_and_id_from_message(
    msg: &PacketMessage<PubrelPacket>,
) -> Option<(PubcompPacket, u16)> {
    let id = msg.packet.packet_identifier();
    Some((PubcompPacket::new(id), id))
}

define_send_packet_actor!(PubrelActor, PublishPacketStatus);
impl_empty_actor!(PubrelActor);
impl_send_packet_actor!(
    PubrelActor,
    PacketMessage<PubrelPacket>,
    PubcompPacket,
    get_retry_count_from_message,
    create_retry_message_from_message,
    create_packet_and_id_from_message,
    |id, _| PacketStatusMessages::RemovePacketStatus(id),
    |status| {
        if let Some(s) = status {
            s.payload == PublishPacketStatus::PendingRel
        } else {
            false
        }
    }
);
