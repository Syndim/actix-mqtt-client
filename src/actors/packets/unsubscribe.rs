use actix::{AsyncContext, Handler, Message};
use log::error;
use mqtt::packet::UnsubscribePacket;
use mqtt::TopicFilter;

use crate::actors::utils;

#[derive(Message)]
pub struct Unsubscribe {
    topic: String,
}

impl Unsubscribe {
    pub fn new(topic: String) -> Self {
        Unsubscribe { topic }
    }
}

#[derive(Message, Clone)]
pub struct BatchUnsubscribe {
    topics: Vec<String>,
    retry_time: u16,
}

impl BatchUnsubscribe {
    pub fn new(topics: Vec<String>) -> Self {
        BatchUnsubscribe {
            topics,
            retry_time: 0,
        }
    }
}

fn get_retry_time_from_message(msg: &BatchUnsubscribe) -> u16 {
    msg.retry_time
}

fn create_retry_message_from_message(msg: BatchUnsubscribe) -> BatchUnsubscribe {
    let mut retry_msg = msg;
    retry_msg.retry_time += 1;
    retry_msg
}

fn create_packet_and_id_from_message(msg: &BatchUnsubscribe) -> Option<(UnsubscribePacket, u16)> {
    let subscriptions: Vec<TopicFilter> = msg
        .topics
        .clone()
        .into_iter()
        .map(|s| TopicFilter::new(s))
        .filter(|r| match r {
            Ok(_) => true,
            Err(e) => {
                error!("Failed to parse topic: {}", e);
                false
            }
        })
        .map(|r| r.unwrap())
        .collect();
    if subscriptions.is_empty() {
        error!("No valid topic found");
        return None;
    }

    let id = utils::next_id();
    let unsubscribe_packet = UnsubscribePacket::new(id, subscriptions);
    Some((unsubscribe_packet, id))
}

define_send_packet_actor!(UnsubscribeActor);
impl_empty_actor!(UnsubscribeActor);
impl_send_packet_actor!(
    UnsubscribeActor,
    BatchUnsubscribe,
    UnsubscribePacket,
    get_retry_time_from_message,
    create_retry_message_from_message,
    create_packet_and_id_from_message
);

impl Handler<Unsubscribe> for UnsubscribeActor {
    type Result = ();
    fn handle(&mut self, msg: Unsubscribe, ctx: &mut Self::Context) -> Self::Result {
        let batch_msg = BatchUnsubscribe::new(vec![msg.topic]);
        ctx.notify(batch_msg);
    }
}
