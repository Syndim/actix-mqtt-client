use std::collections::HashMap;

use actix::{Handler, Message, Recipient};

use crate::actors::packets::disconnect::PacketSendStatus;

pub enum PacketStatusMessages<T> {
    SetPacketStatus(u16, PacketStatus<T>),
    GetPacketStatus(u16),
    RemovePacketStatus(u16),
}

pub struct PacketStatus<T> {
    pub id: u16,
    pub retry_time: u16,
    pub payload: T,
}

impl<T: 'static> Message for PacketStatusMessages<T> {
    type Result = Option<PacketStatus<T>>;
}

pub struct PacketStatusActor<T> {
    packet_status_map: HashMap<u16, PacketStatus<T>>,
    packet_send_status_recipient_option: Option<Recipient<PacketSendStatus>>,
}

impl<T> PacketStatusActor<T> {
    pub fn new(packet_send_status_recipient_option: Option<Recipient<PacketSendStatus>>) -> Self {
        PacketStatusActor {
            packet_status_map: HashMap::new(),
            packet_send_status_recipient_option,
        }
    }
}

impl_generic_empty_actor!(PacketStatusActor);
impl_generic_stop_handler!(PacketStatusActor);

impl<T: 'static> Handler<PacketStatusMessages<T>> for PacketStatusActor<T> {
    type Result = Option<PacketStatus<T>>;
    fn handle(&mut self, msg: PacketStatusMessages<T>, _: &mut Self::Context) -> Self::Result {
        let result = match msg {
            PacketStatusMessages::SetPacketStatus(id, status) => {
                self.packet_status_map.insert(id, status);
                None
            }
            PacketStatusMessages::GetPacketStatus(id) => {
                if self.packet_status_map.contains_key(&id) {
                    self.packet_status_map.remove(&id)
                } else {
                    None
                }
            }
            PacketStatusMessages::RemovePacketStatus(id) => {
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
