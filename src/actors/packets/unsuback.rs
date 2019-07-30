get_packet_identifier_func!(get_id, mqtt::packet::UnsubackPacket);
get_retry_msg_func!(
    get_retry_msg_from_msg,
    crate::actors::packets::PacketMessage<mqtt::packet::UnsubackPacket>
);
define_response_packet_actor!(UnsubackActor);
impl_response_packet_actor!(
    UnsubackActor,
    UnsubackPacket,
    get_id,
    get_retry_msg_from_msg
);
