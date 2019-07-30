fn get_id(_: &mqtt::packet::PingrespPacket) -> u16 {
    0
}

get_retry_msg_func!(
    get_retry_msg_from_msg,
    crate::actors::packets::PacketMessage<mqtt::packet::PingrespPacket>
);

define_response_packet_actor!(PingrespActor);
impl_response_packet_actor!(
    PingrespActor,
    PingrespPacket,
    get_id,
    get_retry_msg_from_msg
);
