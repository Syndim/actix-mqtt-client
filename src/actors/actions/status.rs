use std::collections::HashMap;

use actix::{Handler, Message, Recipient};

use crate::actors::packets::disconnect::PacketSendStatus;

pub enum StatusOperationMessage<T> {
    SetPacketStatus(u16, PacketStatus<T>),
    GetAndRemovePacketStatus(u16),
    RemovePacketStatus(u16),
}

pub struct PacketStatus<T> {
    pub id: u16,
    pub retry_count: u16,
    pub payload: T,
}

impl<T: 'static> Message for StatusOperationMessage<T> {
    type Result = Option<PacketStatus<T>>;
}

#[derive(Message)]
#[rtype(result = "bool")]
pub struct StatusExistenceMessage(pub u16);

pub struct PacketStatusActor<T> {
    name: &'static str,
    packet_status_map: HashMap<u16, PacketStatus<T>>,
    packet_send_status_recipient_option: Option<Recipient<PacketSendStatus>>,
}

impl<T> PacketStatusActor<T> {
    pub fn new(
        name: &'static str,
        packet_send_status_recipient_option: Option<Recipient<PacketSendStatus>>,
    ) -> Self {
        PacketStatusActor {
            name,
            packet_status_map: HashMap::new(),
            packet_send_status_recipient_option,
        }
    }
}

impl_generic_empty_actor!(PacketStatusActor);
impl_generic_stop_handler!(PacketStatusActor);

impl<T: Unpin + 'static> Handler<StatusExistenceMessage> for PacketStatusActor<T> {
    type Result = bool;
    fn handle(&mut self, msg: StatusExistenceMessage, _: &mut Self::Context) -> Self::Result {
        self.packet_status_map.contains_key(&msg.0)
    }
}

impl<T: Unpin + 'static> Handler<StatusOperationMessage<T>> for PacketStatusActor<T> {
    type Result = Option<PacketStatus<T>>;
    fn handle(&mut self, msg: StatusOperationMessage<T>, _: &mut Self::Context) -> Self::Result {
        let result = match msg {
            StatusOperationMessage::SetPacketStatus(id, status) => {
                self.packet_status_map.insert(id, status);
                None
            }
            StatusOperationMessage::GetAndRemovePacketStatus(id) => {
                if self.packet_status_map.contains_key(&id) {
                    self.packet_status_map.remove(&id)
                } else {
                    None
                }
            }
            StatusOperationMessage::RemovePacketStatus(id) => {
                self.packet_status_map.remove(&id);
                None
            }
        };

        if let Some(ref send_status_recipient) = self.packet_send_status_recipient_option {
            let _ = send_status_recipient.do_send(PacketSendStatus {
                finished: self.packet_status_map.len() == 0,
            });
        }

        result
    }
}
