macro_rules! assert_valid_retry_count {
    ($name:expr, $self:ident, $last_retry_count:expr, $id:expr) => {
        if $last_retry_count > crate::consts::MAX_RETRY_COUNT {
            log::error!("Max retry count excceeded");
            if let Err(e) = $self.status_recipient.do_send(
                crate::actors::actions::status::PacketStatusMessages::RemovePacketStatus($id),
            ) {
                crate::actors::handle_send_error(
                    concat!(stringify!($name), ":status_recipient"),
                    e,
                    &$self.error_recipient,
                    &$self.stop_recipient,
                );
            }

            return;
        }
    };
}

macro_rules! define_response_packet_actor {
    ($name:ident) => {
        define_response_packet_actor!($name, ());
    };
    ($name:ident, $status_payload_type:ty) => {
        pub struct $name {
            status_recipient: actix::Recipient<
                crate::actors::actions::status::PacketStatusMessages<$status_payload_type>,
            >,
            error_recipient: actix::Recipient<crate::actors::ErrorMessage>,
            stop_recipient: actix::Recipient<crate::actors::StopMessage>,
        }

        impl $name {
            pub fn new(
                status_recipient: actix::Recipient<
                    crate::actors::actions::status::PacketStatusMessages<$status_payload_type>,
                >,
                error_recipient: actix::Recipient<crate::actors::ErrorMessage>,
                stop_recipient: actix::Recipient<crate::actors::StopMessage>,
            ) -> Self {
                $name {
                    status_recipient,
                    error_recipient,
                    stop_recipient,
                }
            }
        }
    };
}

macro_rules! impl_response_packet_actor {
    ($name:ident, $packet:ident, $get_id_from_packet:ident, $get_retry_msg_from_msg:ident) => {
        impl_empty_actor!($name);
        impl_stop_handler!($name);

        impl actix::Handler<crate::actors::packets::PacketMessage<mqtt::packet::$packet>>
            for $name
        {
            type Result = ();
            fn handle(
                &mut self,
                msg: crate::actors::packets::PacketMessage<mqtt::packet::$packet>,
                ctx: &mut Self::Context,
            ) -> Self::Result {
                log::info!(concat!("Handling message for ", stringify!($name)));
                let id = $get_id_from_packet(&msg.packet);
                crate::actors::packets::reset_packet_status(
                    stringify!($name),
                    ctx,
                    &self.status_recipient,
                    &self.error_recipient,
                    &self.stop_recipient,
                    id,
                    $get_retry_msg_from_msg(msg),
                );
            }
        }
    };
}

macro_rules! get_packet_identifier_func {
    ($name:ident, $packet_type: ty) => {
        fn $name(packet: &$packet_type) -> u16 {
            packet.packet_identifier()
        }
    };
}

macro_rules! get_retry_msg_func {
    ($name:ident, $msg_type: ty) => {
        fn $name(msg: $msg_type) -> $msg_type {
            let mut resend_msg = msg;
            resend_msg.retry_count += 1;
            resend_msg
        }
    };
}

macro_rules! define_send_packet_actor {
    ($name:ident) => {
        define_send_packet_actor!($name, ());
    };
    ($name:ident, $status_paylod_type:ty) => {
        pub struct $name {
            status_recipient: actix::Recipient<
                crate::actors::actions::status::PacketStatusMessages<$status_paylod_type>,
            >,
            send_recipient: actix::Recipient<crate::actors::packets::VariablePacketMessage>,
            error_recipient: actix::Recipient<crate::actors::ErrorMessage>,
            stop_recipient: actix::Recipient<crate::actors::StopMessage>,
        }

        impl $name {
            pub fn new(
                status_recipient: actix::Recipient<
                    crate::actors::actions::status::PacketStatusMessages<$status_paylod_type>,
                >,
                send_recipient: actix::Recipient<crate::actors::packets::VariablePacketMessage>,
                error_recipient: actix::Recipient<crate::actors::ErrorMessage>,
                stop_recipient: actix::Recipient<crate::actors::StopMessage>,
            ) -> Self {
                $name {
                    status_recipient,
                    send_recipient,
                    error_recipient,
                    stop_recipient,
                }
            }
        }
    };
}

macro_rules! impl_send_packet_actor {
    ($name:ident, $message:ty, $packet:ident, $get_retry_count_from_message:ident, $create_retry_msessage_from_message:ident, $create_packet_and_id_from_message:ident) => {
        impl_send_packet_actor!(
            $name,
            $message,
            $packet,
            $get_retry_count_from_message,
            $create_retry_msessage_from_message,
            $create_packet_and_id_from_message,
            |id, retry_count| crate::actors::actions::status::PacketStatusMessages::SetPacketStatus(
                id,
                crate::actors::actions::status::PacketStatus {
                    id,
                    retry_count,
                    payload: ()
                }
            ),
            |status| status.is_some()
        );
    };
    ($name:ident, $message:ty, $packet:ident, $get_retry_count_from_message:ident, $create_retry_msessage_from_message:ident, $create_packet_and_id_from_message:ident, $create_status_from_id_and_retry_count:expr, $status_check_func:expr) => {
        impl_stop_handler!($name);

        impl actix::Handler<$message> for $name {
            type Result = ();
            fn handle(&mut self, msg: $message, ctx: &mut Self::Context) -> Self::Result {
                log::info!(concat!("Handling message for ", stringify!($name)));
                let packet_and_id_option = $create_packet_and_id_from_message(&msg);
                if packet_and_id_option.is_none() {
                    return;
                }

                let (packet, id) = packet_and_id_option.unwrap();
                let last_retry_count = $get_retry_count_from_message(&msg);
                assert_valid_retry_count!($name, self, last_retry_count, id);

                let resend_msg = $create_retry_msessage_from_message(msg);
                let status_msg = $create_status_from_id_and_retry_count(id, last_retry_count);
                if !crate::actors::packets::set_packet_status(
                    stringify!($name),
                    ctx,
                    &self.status_recipient,
                    &self.error_recipient,
                    &self.stop_recipient,
                    resend_msg.clone(),
                    status_msg,
                ) {
                    return;
                }

                if !crate::actors::packets::send_packet(
                    stringify!($name),
                    ctx,
                    &self.send_recipient,
                    &self.error_recipient,
                    &self.stop_recipient,
                    mqtt::packet::VariablePacket::$packet(packet),
                    resend_msg.clone(),
                ) {
                    return;
                }

                crate::actors::packets::schedule_status_check(
                    ctx,
                    &self.status_recipient,
                    &self.error_recipient,
                    &self.stop_recipient,
                    id,
                    resend_msg,
                    $status_check_func,
                );
            }
        }
    };
}
