use std::vec::Vec;

use actix::{AsyncContext, Handler, Message};
use log::error;
use mqtt::packet::SubscribePacket;
pub use mqtt::{QualityOfService, TopicFilter};

use crate::actors::utils;

#[derive(Message, Clone)]
pub struct Subscribe {
    topic: String,
    qos: QualityOfService,
}

impl Subscribe {
    pub fn new(topic: String, qos: QualityOfService) -> Self {
        Subscribe { topic, qos }
    }
}

#[derive(Message, Clone)]
pub struct BatchSubscribe {
    subscriptions: Vec<Subscribe>,
    retry_time: u16,
}

impl BatchSubscribe {
    pub fn new(subscriptions: Vec<Subscribe>) -> Self {
        BatchSubscribe {
            subscriptions,
            retry_time: 0,
        }
    }
}

fn get_retry_time_from_message(msg: &BatchSubscribe) -> u16 {
    msg.retry_time
}

fn create_retry_message_from_message(msg: BatchSubscribe) -> BatchSubscribe {
    let mut retry_msg = msg;
    retry_msg.retry_time += 1;
    retry_msg
}

fn create_packet_and_id_from_message(msg: &BatchSubscribe) -> Option<(SubscribePacket, u16)> {
    let subscriptions: Vec<(TopicFilter, QualityOfService)> = msg
        .subscriptions
        .clone()
        .into_iter()
        .map(|s| (TopicFilter::new(s.topic), s.qos))
        .filter(|(result, _)| match result {
            Ok(_) => true,
            Err(e) => {
                error!("Error pasing topic: {}, ignore", e);
                false
            }
        })
        .map(|(result, qos)| (result.unwrap(), qos))
        .collect();
    if subscriptions.is_empty() {
        error!("No valid topic found");
        return None;
    }

    let id = utils::next_id();
    let subscribe_packet = SubscribePacket::new(id, subscriptions);
    Some((subscribe_packet, id))
}

define_send_packet_actor!(SubscribeActor);
impl_empty_actor!(SubscribeActor);
impl_send_packet_actor!(
    SubscribeActor,
    BatchSubscribe,
    SubscribePacket,
    get_retry_time_from_message,
    create_retry_message_from_message,
    create_packet_and_id_from_message
);

impl Handler<Subscribe> for SubscribeActor {
    type Result = ();
    fn handle(&mut self, msg: Subscribe, ctx: &mut Self::Context) -> Self::Result {
        let batch_msg = BatchSubscribe::new(vec![msg]);
        ctx.notify(batch_msg);
    }
}
