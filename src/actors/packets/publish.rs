use std::io::ErrorKind;
use std::time::Instant;

use actix::{Arbiter, AsyncContext, Handler, Message, Recipient};
use futures::Future;
use log::info;
use mqtt::packet::{
    Packet, PubackPacket, PublishPacket, PubrecPacket, QoSWithPacketIdentifier, VariablePacket,
};
use mqtt::{QualityOfService, TopicName};
use tokio::timer::Delay;

use crate::actors::actions::status::{PacketStatus, PacketStatusMessages};
use crate::actors::utils;
use crate::actors::{
    handle_mailbox_error_with_resend, handle_send_error, handle_send_error_with_resend,
    ErrorMessage, StopMessage,
};
use crate::consts::COMMAND_TIMEOUT;

use super::{
    schedule_status_check, send_error, PacketMessage, PublishMessage, PublishPacketStatus,
    VariablePacketMessage,
};

#[derive(Message, Clone)]
pub struct Publish {
    topic: String,
    qos: QualityOfService,
    payload: Vec<u8>,
    retry_time: u16,
}

impl Publish {
    pub fn new(topic: String, qos: QualityOfService, payload: Vec<u8>) -> Self {
        Publish {
            topic,
            qos,
            payload,
            retry_time: 0,
        }
    }
}

fn create_packet_and_id_from_message(
    msg: &Publish,
    error_recipient: &Recipient<ErrorMessage>,
) -> Option<(PublishPacket, u16)> {
    match TopicName::new(msg.topic.clone()) {
        Ok(topic) => {
            let id = if msg.qos == QualityOfService::Level0 {
                0
            } else {
                utils::next_id()
            };
            let packet = PublishPacket::new(
                topic,
                QoSWithPacketIdentifier::new(msg.qos, id),
                msg.payload.clone(),
            );
            Some((packet, id))
        }
        Err(e) => {
            send_error(
                error_recipient,
                ErrorKind::InvalidInput,
                format!("Failed to create topic from {}, error: {}", &*msg.topic, e),
            );
            None
        }
    }
}

pub struct SendPublishActor {
    status_recipient: Recipient<PacketStatusMessages<PublishPacketStatus>>,
    send_recipient: Recipient<VariablePacketMessage>,
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
}

impl SendPublishActor {
    pub fn new(
        status_recipient: Recipient<PacketStatusMessages<PublishPacketStatus>>,
        send_recipient: Recipient<VariablePacketMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
    ) -> Self {
        SendPublishActor {
            status_recipient,
            send_recipient,
            error_recipient,
            stop_recipient,
        }
    }
}

impl_empty_actor!(SendPublishActor);
impl_stop_handler!(SendPublishActor);

impl Handler<Publish> for SendPublishActor {
    type Result = ();
    fn handle(&mut self, msg: Publish, ctx: &mut Self::Context) -> Self::Result {
        info!("Handle message for SendPublishActor");
        assert_valid_retry_time!(self, msg.retry_time, 0);
        let packet_and_id_option = create_packet_and_id_from_message(&msg, &self.error_recipient);
        if packet_and_id_option.is_none() {
            return;
        }

        let (packet, id) = packet_and_id_option.unwrap();
        let variable_packet = VariablePacket::PublishPacket(packet);
        let variable_message = VariablePacketMessage::new(variable_packet, 0);

        match msg.qos {
            QualityOfService::Level0 => {
                if let Err(e) = self.send_recipient.try_send(variable_message) {
                    handle_send_error(e, &self.error_recipient, &self.stop_recipient);
                    return;
                }
            }
            QualityOfService::Level1 => {
                let mut resend_msg = msg.clone();
                resend_msg.retry_time += 1;

                if let Err(e) =
                    self.status_recipient
                        .try_send(PacketStatusMessages::SetPacketStatus(
                            id,
                            PacketStatus {
                                id,
                                retry_time: msg.retry_time,
                                payload: PublishPacketStatus::PendingAck,
                            },
                        ))
                {
                    handle_send_error_with_resend(
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                        ctx.address(),
                        resend_msg,
                    );
                    return;
                }

                if let Err(e) = self.send_recipient.try_send(variable_message) {
                    handle_send_error_with_resend(
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                        ctx.address(),
                        resend_msg,
                    );
                    return;
                }

                schedule_status_check(
                    ctx,
                    &self.status_recipient,
                    &self.error_recipient,
                    &self.stop_recipient,
                    id,
                    resend_msg,
                    |status| status.is_some(),
                );
            }
            QualityOfService::Level2 => {
                let mut resend_msg = msg.clone();
                resend_msg.retry_time += 1;

                if let Err(e) =
                    self.status_recipient
                        .try_send(PacketStatusMessages::SetPacketStatus(
                            id,
                            PacketStatus {
                                id,
                                retry_time: msg.retry_time,
                                payload: PublishPacketStatus::PendingRec,
                            },
                        ))
                {
                    handle_send_error_with_resend(
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                        ctx.address(),
                        resend_msg,
                    );
                    return;
                }

                if let Err(e) = self.send_recipient.try_send(variable_message) {
                    handle_send_error_with_resend(
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                        ctx.address(),
                        resend_msg,
                    );
                    return;
                }

                let addr = ctx.address();
                let addr_clone = addr.clone();
                let msg_clone = resend_msg.clone();
                let error_recipient = self.error_recipient.clone();
                let stop_recipient = self.stop_recipient.clone();
                ctx.run_later(COMMAND_TIMEOUT.clone(), move |actor, _| {
                    Arbiter::spawn(
                        actor
                            .status_recipient
                            .send(PacketStatusMessages::GetPacketStatus(id))
                            .map(move |status| {
                                if let Some(s) = status {
                                    if s.payload == PublishPacketStatus::PendingRec {
                                        addr.do_send(resend_msg);
                                    }
                                }
                            })
                            .map_err(move |e| {
                                handle_mailbox_error_with_resend(
                                    e,
                                    &error_recipient,
                                    &stop_recipient,
                                    addr_clone,
                                    msg_clone,
                                );
                            }),
                    );
                });
            }
        }
    }
}

pub struct RecvPublishActor {
    status_recipient: Recipient<PacketStatusMessages<PublishPacketStatus>>,
    send_recipient: Recipient<VariablePacketMessage>,
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
    remote_message_recipient: Recipient<PublishMessage>,
}

impl RecvPublishActor {
    pub fn new(
        status_recipient: Recipient<PacketStatusMessages<PublishPacketStatus>>,
        send_recipient: Recipient<VariablePacketMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
        remote_message_recipient: Recipient<PublishMessage>,
    ) -> Self {
        RecvPublishActor {
            status_recipient,
            send_recipient,
            error_recipient,
            stop_recipient,
            remote_message_recipient,
        }
    }
}

impl_empty_actor!(RecvPublishActor);
impl_stop_handler!(RecvPublishActor);

impl Handler<PacketMessage<PublishPacket>> for RecvPublishActor {
    type Result = ();
    fn handle(
        &mut self,
        msg: PacketMessage<PublishPacket>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("Handle message for RecvPublishActor");
        let packet = &msg.packet;
        match packet.qos() {
            QoSWithPacketIdentifier::Level0 => {
                assert_valid_retry_time!(self, msg.retry_time, 0);
                let publish_message = PublishMessage {
                    id: 0,
                    topic_name: String::from(packet.topic_name()),
                    payload: msg.packet.payload(),
                };
                if let Err(e) = self
                    .status_recipient
                    .do_send(PacketStatusMessages::RemovePacketStatus(0))
                {
                    handle_send_error(e, &self.error_recipient, &self.stop_recipient);
                }

                if let Err(e) = self.remote_message_recipient.try_send(publish_message) {
                    handle_send_error(e, &self.error_recipient, &self.stop_recipient);
                }
            }
            QoSWithPacketIdentifier::Level1(id) => {
                assert_valid_retry_time!(self, msg.retry_time, id);
                let mut resend_msg = msg.clone();
                resend_msg.retry_time += 1;

                let puback_packet = PubackPacket::new(id);
                let packet_message =
                    VariablePacketMessage::new(VariablePacket::PubackPacket(puback_packet), 0);
                if let Err(e) = self.send_recipient.try_send(packet_message) {
                    handle_send_error_with_resend(
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                        ctx.address(),
                        resend_msg,
                    );

                    return;
                }

                let publish_message = PublishMessage {
                    id,
                    topic_name: String::from(packet.topic_name()),
                    payload: msg.packet.payload(),
                };
                if let Err(e) = self.remote_message_recipient.try_send(publish_message) {
                    handle_send_error_with_resend(
                        e,
                        &self.error_recipient,
                        &self.stop_recipient,
                        ctx.address(),
                        resend_msg,
                    );
                }
            }
            QoSWithPacketIdentifier::Level2(id) => {
                assert_valid_retry_time!(self, msg.retry_time, id);
                let mut resend_msg = msg.clone();
                resend_msg.retry_time += 1;
                let addr = ctx.address();
                let addr_clone = addr.clone();
                let msg_clone = resend_msg.clone();
                let status_recipient = self.status_recipient.clone();
                let error_recipient = self.error_recipient.clone();
                let error_recipient_clone = error_recipient.clone();
                let stop_recipient = self.stop_recipient.clone();
                let stop_recipient_clone = stop_recipient.clone();
                let send_recipient = self.send_recipient.clone();
                let remote_message_recipient = self.remote_message_recipient.clone();
                let packet = msg.packet;
                let retry_time = msg.retry_time;
                Arbiter::spawn(
                    self.status_recipient
                        .send(PacketStatusMessages::GetPacketStatus(id))
                        .map(move |status| {
                            if status.is_none() && retry_time == 0 {
                                let publish_message = PublishMessage {
                                    id,
                                    topic_name: String::from(packet.topic_name()),
                                    payload: packet.payload(),
                                };
                                if let Err(e) = remote_message_recipient.try_send(publish_message) {
                                    let mut resend_msg_for_publish = resend_msg;
                                    resend_msg_for_publish.retry_time -= 1;
                                    handle_send_error_with_resend(
                                        e,
                                        &error_recipient,
                                        &stop_recipient,
                                        addr,
                                        resend_msg_for_publish,
                                    );
                                    return;
                                }
                            }

                            if let Err(e) =
                                status_recipient.try_send(PacketStatusMessages::SetPacketStatus(
                                    id,
                                    PacketStatus {
                                        id,
                                        retry_time,
                                        payload: PublishPacketStatus::PendingRel,
                                    },
                                ))
                            {
                                handle_send_error_with_resend(
                                    e,
                                    &error_recipient,
                                    &stop_recipient,
                                    addr,
                                    resend_msg,
                                );

                                return;
                            }

                            let pubrec_packet = PubrecPacket::new(id);
                            let packet_message = VariablePacketMessage::new(
                                VariablePacket::PubrecPacket(pubrec_packet),
                                0,
                            );
                            if let Err(e) = send_recipient.try_send(packet_message) {
                                handle_send_error_with_resend(
                                    e,
                                    &error_recipient,
                                    &stop_recipient,
                                    addr,
                                    resend_msg,
                                );

                                return;
                            }

                            let command_deadline = Instant::now() + COMMAND_TIMEOUT.clone();
                            let error_recipient_clone = error_recipient.clone();
                            let addr_clone = addr.clone();
                            let resend_msg_clone = resend_msg.clone();
                            Arbiter::spawn(
                                Delay::new(command_deadline)
                                    .map(move |_| {
                                        Arbiter::spawn(
                                            status_recipient
                                                .send(PacketStatusMessages::GetPacketStatus(id))
                                                .map(move |status| {
                                                    if let Some(s) = status {
                                                        if s.payload
                                                            == PublishPacketStatus::PendingRec
                                                        {
                                                            addr.do_send(resend_msg);
                                                        }
                                                    }
                                                })
                                                .map_err(move |e| {
                                                    handle_mailbox_error_with_resend(
                                                        e,
                                                        &error_recipient_clone,
                                                        &stop_recipient,
                                                        addr_clone,
                                                        resend_msg_clone,
                                                    );
                                                }),
                                        );
                                    })
                                    .map_err(move |e| {
                                        send_error(
                                            &error_recipient,
                                            ErrorKind::TimedOut,
                                            format!("Failed to create delay: {}", e),
                                        )
                                    }),
                            )
                        })
                        .map_err(move |e| {
                            handle_mailbox_error_with_resend(
                                e,
                                &error_recipient_clone,
                                &stop_recipient_clone,
                                addr_clone,
                                msg_clone,
                            );
                        }),
                );
            }
        }
    }
}
