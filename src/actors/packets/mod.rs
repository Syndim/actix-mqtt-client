#[macro_use]
mod macros;
pub mod connack;
pub mod connect;
pub mod disconnect;
pub mod pingreq;
pub mod pingresp;
pub mod puback;
pub mod pubcomp;
pub mod publish;
pub mod pubrec;
pub mod pubrel;
pub mod suback;
pub mod subscribe;
pub mod unsuback;
pub mod unsubscribe;

use std::vec::Vec;

use actix::dev::ToEnvelope;
use actix::{Actor, AsyncContext, Context, Handler, Message, Recipient};
use mqtt::packet::VariablePacket;

use crate::actors::actions::status::{PacketStatus, PacketStatusMessages};
use crate::actors::{ErrorMessage, StopMessage};
use crate::consts::COMMAND_TIMEOUT;

use super::{handle_mailbox_error_with_resend, handle_send_error_with_resend, send_error};

#[derive(Clone)]
pub struct PacketMessage<T: Clone> {
    pub packet: T,
    pub retry_count: u16,
}

impl<T: Clone> Message for PacketMessage<T> {
    type Result = ();
}

impl<T: Clone> PacketMessage<T> {
    pub fn new(packet: T, retry_count: u16) -> Self {
        PacketMessage {
            packet,
            retry_count,
        }
    }
}

pub type VariablePacketMessage = PacketMessage<VariablePacket>;

/// The actix message containing the payload of a MQTT publish packet
#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct PublishMessage {
    /// The packet identifier of the publish packet for QoS Level 1 and Level 2, or 0 for QoS Level 0
    pub id: u16,
    /// The topic name of the message
    pub topic_name: String,
    /// The message payload
    pub payload: Vec<u8>,
}

#[derive(PartialEq)]
pub enum PublishPacketStatus {
    PendingAck,
    PendingRec,
    PendingRel,
    PendingComp,
}

fn schedule_status_check<TActor, TMessage, TStatusPayload, TStatusCheckFunc>(
    ctx: &mut Context<TActor>,
    status_recipient: &Recipient<PacketStatusMessages<TStatusPayload>>,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
    id: u16,
    retry_msg: TMessage,
    status_check_func: TStatusCheckFunc,
) where
    TActor: Actor<Context = Context<TActor>> + Handler<TMessage>,
    TMessage: Message + Send + 'static + Clone,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
    TStatusPayload: Send,
    TStatusCheckFunc: FnOnce(&Option<PacketStatus<TStatusPayload>>) -> bool + 'static,
{
    let error_recipient = error_recipient.clone();
    let stop_recipient = stop_recipient.clone();
    let status_recipient = status_recipient.clone();
    let addr = ctx.address();
    let addr_clone = addr.clone();
    let msg_clone = retry_msg.clone();
    ctx.run_later(COMMAND_TIMEOUT.clone(), move |_, _| {
        let status_future = async move {
            let status_result = status_recipient
                .send(crate::actors::actions::status::PacketStatusMessages::GetPacketStatus(id))
                .await;
            match status_result {
                Ok(status) => {
                    if status_check_func(&status) {
                        addr.do_send(retry_msg);
                    }
                }
                Err(e) => {
                    handle_mailbox_error_with_resend(
                        "schedule_status_check",
                        e,
                        &error_recipient,
                        &stop_recipient,
                        addr_clone,
                        msg_clone,
                    );
                }
            }
        };

        actix::Arbiter::spawn(status_future);
    });
}

fn set_packet_status<TActor, TMessage, TStatusPayload>(
    tag: &str,
    ctx: &mut Context<TActor>,
    status_recipient: &Recipient<PacketStatusMessages<TStatusPayload>>,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
    resend_msg: TMessage,
    status: PacketStatusMessages<TStatusPayload>,
) -> bool
where
    TActor: Actor<Context = Context<TActor>> + Handler<TMessage>,
    TMessage: Message + Send + 'static,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
    TStatusPayload: Send,
{
    if let Err(e) = status_recipient.try_send(status) {
        let addr = ctx.address();
        handle_send_error_with_resend(tag, e, error_recipient, stop_recipient, addr, resend_msg);
        false
    } else {
        true
    }
}

fn reset_packet_status<TActor, TMessage, TStatusPayload>(
    tag: &str,
    ctx: &mut Context<TActor>,
    status_recipient: &Recipient<PacketStatusMessages<TStatusPayload>>,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
    id: u16,
    resend_msg: TMessage,
) -> bool
where
    TActor: Actor<Context = Context<TActor>> + Handler<TMessage>,
    TMessage: Message + Send + 'static,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
    TStatusPayload: Send,
{
    if let Err(e) = status_recipient.try_send(PacketStatusMessages::RemovePacketStatus(id)) {
        let addr = ctx.address();
        handle_send_error_with_resend(tag, e, error_recipient, stop_recipient, addr, resend_msg);
        false
    } else {
        true
    }
}

fn send_packet<TActor, TMessage>(
    tag: &str,
    ctx: &Context<TActor>,
    send_recipient: &Recipient<VariablePacketMessage>,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
    packet: VariablePacket,
    resend_msg: TMessage,
) -> bool
where
    TActor: Actor<Context = Context<TActor>> + Handler<TMessage>,
    TMessage: Message + Send + 'static,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
{
    let message = VariablePacketMessage::new(packet, 0);
    if let Err(e) = send_recipient.try_send(message) {
        let addr = ctx.address();
        handle_send_error_with_resend(tag, e, error_recipient, stop_recipient, addr, resend_msg);
        false
    } else {
        true
    }
}
