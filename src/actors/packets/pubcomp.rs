get_packet_identifier_func!(get_id, mqtt::packet::PubcompPacket);
get_retry_msg_func!(
    get_retry_msg_from_msg,
    crate::actors::packets::PacketMessage<mqtt::packet::PubcompPacket>
);
define_response_packet_actor!(PubcompActor, crate::actors::packets::PublishPacketStatus);
impl_response_packet_actor!(PubcompActor, PubcompPacket, get_id, get_retry_msg_from_msg);
