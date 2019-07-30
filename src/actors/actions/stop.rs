use std::vec::Vec;

use actix::{ActorContext, AsyncContext, Handler, Message, Recipient, System};
use log::info;

use crate::actors::StopMessage;
use crate::consts::DELAY_BEFORE_SHUTDOWN;

#[derive(Message)]
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

        info!(
            "Waiting for {:?} before shutdown the system",
            &*DELAY_BEFORE_SHUTDOWN
        );
        let _ = ctx.run_later(DELAY_BEFORE_SHUTDOWN.clone(), |_, ctx| {
            System::current().stop();
            ctx.stop();
        });
    }
}

impl Handler<AddStopRecipient> for StopActor {
    type Result = ();
    fn handle(&mut self, msg: AddStopRecipient, _: &mut Self::Context) -> Self::Result {
        self.stop_recipients.push(msg.0);
    }
}
