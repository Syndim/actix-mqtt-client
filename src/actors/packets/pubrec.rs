use mqtt::packet::{PubrecPacket, PubrelPacket};

use crate::actors::actions::status::{PacketStatus, PacketStatusMessages};
use crate::actors::packets::{PacketMessage, PublishPacketStatus};

fn get_retry_time_from_message(msg: &PacketMessage<PubrecPacket>) -> u16 {
    msg.retry_time
}

fn create_retry_message_from_message(
    msg: PacketMessage<PubrecPacket>,
) -> PacketMessage<PubrecPacket> {
    let mut retry_msg = msg;
    retry_msg.retry_time += 1;
    retry_msg
}

fn create_packet_and_id_from_message(
    msg: &PacketMessage<PubrecPacket>,
) -> Option<(PubrelPacket, u16)> {
    let id = msg.packet.packet_identifier();
    Some((PubrelPacket::new(id), id))
}

define_send_packet_actor!(PubrecActor, PublishPacketStatus);
impl_empty_actor!(PubrecActor);
impl_send_packet_actor!(
    PubrecActor,
    PacketMessage<PubrecPacket>,
    PubrelPacket,
    get_retry_time_from_message,
    create_retry_message_from_message,
    create_packet_and_id_from_message,
    |id, retry_time| PacketStatusMessages::SetPacketStatus(
        id,
        PacketStatus {
            id,
            retry_time,
            payload: PublishPacketStatus::PendingComp
        }
    ),
    |status| {
        if let Some(s) = status {
            s.payload == PublishPacketStatus::PendingRec
        } else {
            false
        }
    }
);
