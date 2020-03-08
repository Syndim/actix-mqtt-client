use std::vec::Vec;

use actix::{ActorContext, Handler, Message, Recipient};
use log::info;

use crate::actors::StopMessage;

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddStopRecipient(pub Recipient<StopMessage>);

pub struct StopActor {
    stop_recipients: Vec<Recipient<StopMessage>>,
    stopping: bool,
}

impl StopActor {
    pub fn new() -> Self {
        StopActor {
            stop_recipients: Vec::new(),
            stopping: false,
        }
    }
}

impl_empty_actor!(StopActor);

impl Handler<StopMessage> for StopActor {
    type Result = ();
    fn handle(&mut self, _: StopMessage, ctx: &mut Self::Context) -> Self::Result {
        if self.stopping {
            info!("Already stopping");
            return;
        }

        self.stopping = true;
        for stop_recipient in &self.stop_recipients {
            let _ = stop_recipient.do_send(StopMessage);
        }

        ctx.stop();
    }
}

impl Handler<AddStopRecipient> for StopActor {
    type Result = ();
    fn handle(&mut self, msg: AddStopRecipient, _: &mut Self::Context) -> Self::Result {
        self.stop_recipients.push(msg.0);
    }
}
