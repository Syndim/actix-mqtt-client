use std::time::Duration;

use actix::{Actor, Arbiter, AsyncContext, Context, Handler, Message, Recipient};
use log::{error, trace};
use mqtt::packet::PingreqPacket;

use crate::actors::actions::status::{StatusExistenceMessage, StatusOperationMessage};
use crate::actors::{ErrorMessage, StopMessage};

use super::handle_mailbox_error_with_resend;
use super::VariablePacketMessage;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Pingreq(pub u16);

#[derive(Message, Clone)]
#[rtype(result = "()")]
struct SendPing(pub u16);

pub struct PingreqActor {
    status_recipient: Recipient<StatusOperationMessage<()>>,
    connect_status_recipient: Recipient<StatusExistenceMessage>,
    send_recipient: Recipient<VariablePacketMessage>,
    error_recipient: Recipient<ErrorMessage>,
    stop_recipient: Recipient<StopMessage>,
    interval: Duration,
}

impl PingreqActor {
    pub fn new(
        status_recipient: Recipient<StatusOperationMessage<()>>,
        connect_status_recipient: Recipient<StatusExistenceMessage>,
        send_recipient: Recipient<VariablePacketMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
        interval: Duration,
    ) -> Self {
        PingreqActor {
            status_recipient,
            connect_status_recipient,
            send_recipient,
            error_recipient,
            stop_recipient,
            interval,
        }
    }
}

impl Actor for PingreqActor {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        trace!("PingreqActor started");
        ctx.notify(Pingreq(0));
        ctx.run_interval(self.interval.clone(), |_, ctx| {
            trace!("Start to send ping");
            ctx.notify(Pingreq(0));
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        trace!("PingreqActor stopped");
    }
}

impl Handler<Pingreq> for PingreqActor {
    type Result = ();
    fn handle(&mut self, msg: Pingreq, ctx: &mut Self::Context) -> Self::Result {
        let last_retry_count = msg.0;
        assert_valid_retry_count!(PingreqActor, self, last_retry_count, 0);
        let status_recipient = self.status_recipient.clone();
        let connect_status_recipient = self.connect_status_recipient.clone();
        let error_recipient = self.error_recipient.clone();
        let stop_recipient = self.stop_recipient.clone();
        let addr = ctx.address();
        let addr_clone = addr.clone();
        let status_future = async move {
            // For connect status:
            //      status message with id = 0 indicating the connecing status
            //      status message with id = 1 indicating the connected status
            let connect_status_result = connect_status_recipient
                .send(StatusExistenceMessage(1u16))
                .await;
            match connect_status_result {
                Ok(false) => {
                    trace!("Server not connected yet, do nothing.");
                    return;
                }
                Err(e) => {
                    error!("Failed to get connect status: {}", e);
                    return;
                }
                _ => {
                    trace!("Server connected, will send ping");
                }
            }

            let status_result = status_recipient
                .send(StatusOperationMessage::GetAndRemovePacketStatus(0))
                .await;
            match status_result {
                Ok(status) => {
                    if status.is_none() {
                        // Only try send ping if no previous on-going ping
                        addr.do_send(SendPing(0));
                    }
                }
                Err(e) => {
                    handle_mailbox_error_with_resend(
                        "PingreqActor:status_recipient",
                        e,
                        &error_recipient,
                        &stop_recipient,
                        addr_clone,
                        Pingreq(last_retry_count + 1),
                    );
                }
            }
        };
        Arbiter::spawn(status_future);
    }
}

fn get_retry_count_from_message(msg: &SendPing) -> u16 {
    msg.0
}

fn create_retry_msessage_from_message(msg: SendPing) -> SendPing {
    SendPing(msg.0 + 1)
}

fn create_packet_and_id_from_message(_: &SendPing) -> Option<(PingreqPacket, u16)> {
    Some((PingreqPacket::new(), 0))
}

impl_send_packet_actor!(
    PingreqActor,
    SendPing,
    PingreqPacket,
    get_retry_count_from_message,
    create_retry_msessage_from_message,
    create_packet_and_id_from_message
);
