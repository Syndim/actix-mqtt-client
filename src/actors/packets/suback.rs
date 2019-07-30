get_packet_identifier_func!(get_id, mqtt::packet::SubackPacket);
get_retry_msg_func!(
    get_retry_msg_from_msg,
    crate::actors::packets::PacketMessage<mqtt::packet::SubackPacket>
);
define_response_packet_actor!(SubackActor);
impl_response_packet_actor!(SubackActor, SubackPacket, get_id, get_retry_msg_from_msg);
