get_packet_identifier_func!(get_id, mqtt::packet::PubackPacket);
get_retry_msg_func!(
    get_retry_msg_from_msg,
    crate::actors::packets::PacketMessage<mqtt::packet::PubackPacket>
);
define_response_packet_actor!(PubackActor, crate::actors::packets::PublishPacketStatus);
impl_response_packet_actor!(PubackActor, PubackPacket, get_id, get_retry_msg_from_msg);
