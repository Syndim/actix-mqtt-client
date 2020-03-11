use actix::{ActorContext, AsyncContext, Handler, Message, Recipient};
use log::{debug, trace};
use mqtt::packet::{DisconnectPacket, VariablePacket};

use crate::actors::{ErrorMessage, StopMessage};

use super::{send_packet, VariablePacketMessage};

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub force: bool,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct PacketSendStatus {
    pub finished: bool,
}

pub struct DisconnectActor {
    stop_recipient: Recipient<StopMessage>,
    error_recipient: Recipient<ErrorMessage>,
    send_recipient: Recipient<VariablePacketMessage>,
    packet_send_finished: bool,
    pending_disconnect: bool,
}

impl DisconnectActor {
    pub fn new(
        send_recipient: Recipient<VariablePacketMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Recipient<StopMessage>,
    ) -> Self {
        DisconnectActor {
            send_recipient,
            error_recipient,
            stop_recipient,
            packet_send_finished: true,
            pending_disconnect: false,
        }
    }
}

impl_empty_actor!(DisconnectActor);
impl_stop_handler!(DisconnectActor);

impl Handler<Disconnect> for DisconnectActor {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, ctx: &mut Self::Context) -> Self::Result {
        trace!("Handle message for DisconnectActor");
        // Need to wait for all pending packets to be sent before sending disconnect to server
        if msg.force || self.packet_send_finished {
            debug!(
                "Sending disconnect to sever, force: {}, no pending packet: {}",
                msg.force, self.packet_send_finished
            );
            let packet = VariablePacket::DisconnectPacket(DisconnectPacket::new());
            send_packet(
                "DisconnectActor::disconnect",
                ctx,
                &self.send_recipient,
                &self.error_recipient,
                &self.stop_recipient,
                packet,
                msg,
            );

            let _ = self.stop_recipient.do_send(StopMessage);
            ctx.stop();
        } else {
            trace!(
                "Has pending packet, wait for them to be sent before sending disconnect to server"
            );
            self.pending_disconnect = true;
        }
    }
}

impl Handler<PacketSendStatus> for DisconnectActor {
    type Result = ();
    fn handle(&mut self, msg: PacketSendStatus, ctx: &mut Self::Context) -> Self::Result {
        self.packet_send_finished = msg.finished;
        if self.pending_disconnect && self.packet_send_finished {
            self.pending_disconnect = false;
            ctx.address().do_send(Disconnect { force: true });
        }
    }
}
