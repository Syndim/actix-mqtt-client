use std::boxed::Box;
use std::io::{Error as IoError, ErrorKind, Write};

use actix::{Actor, Addr, MailboxError, Recipient};
use futures::{future, Future};
use mqtt::QualityOfService;
use tokio::io::AsyncRead;

use crate::actors::actions::dispatch::DispatchActor;
use crate::actors::actions::recv::RecvActor;
use crate::actors::actions::send::SendActor;
use crate::actors::actions::status::PacketStatusActor;
use crate::actors::actions::stop::{AddStopRecipient, StopActor};
use crate::actors::packets::connack::ConnackActor;
use crate::actors::packets::connect::{Connect, ConnectActor};
use crate::actors::packets::disconnect::{Disconnect, DisconnectActor};
use crate::actors::packets::pingreq::PingreqActor;
use crate::actors::packets::pingresp::PingrespActor;
use crate::actors::packets::puback::PubackActor;
use crate::actors::packets::pubcomp::PubcompActor;
use crate::actors::packets::publish::{Publish, RecvPublishActor, SendPublishActor};
use crate::actors::packets::pubrec::PubrecActor;
use crate::actors::packets::pubrel::PubrelActor;
use crate::actors::packets::suback::SubackActor;
use crate::actors::packets::subscribe::{Subscribe, SubscribeActor};
use crate::actors::packets::unsuback::UnsubackActor;
use crate::actors::packets::unsubscribe::{Unsubscribe, UnsubscribeActor};
use crate::actors::packets::{PublishMessage, PublishPacketStatus};
use crate::actors::ErrorMessage;
use crate::consts::PING_INTERVAL;

#[inline]
fn map_mailbox_error_to_io_error(e: MailboxError) -> IoError {
    IoError::new(ErrorKind::Interrupted, format!("{}", e))
}

#[inline]
fn address_not_found_error(name: &str) -> IoError {
    IoError::new(ErrorKind::NotFound, format!("{} address not found", name))
}

/// The options for setting up MQTT connection
#[derive(Default, Clone)]
pub struct MqttOptions {
    /// User name
    pub user_name: Option<String>,
    /// Password
    pub password: Option<String>,
    /// Keep alive time(in seconds)
    pub keep_alive: Option<u16>,
}

/// The client for connecting to the MQTT server
#[derive(Clone)]
pub struct MqttClient {
    conn_addr: Option<Addr<ConnectActor>>,
    pub_addr: Option<Addr<SendPublishActor>>,
    sub_addr: Option<Addr<SubscribeActor>>,
    unsub_addr: Option<Addr<UnsubscribeActor>>,
    stop_addr: Option<Addr<StopActor>>,
    disconnect_addr: Option<Addr<DisconnectActor>>,
    client_name: String,
    options: MqttOptions,
}

impl MqttClient {
    /// Create a new MQTT client
    pub fn new<TReader: AsyncRead + Send + 'static, TWriter: Write + Send + 'static>(
        reader: TReader,
        writer: TWriter,
        client_name: String,
        options: MqttOptions,
        message_recipient: Recipient<PublishMessage>,
        error_recipient: Recipient<ErrorMessage>,
    ) -> Self {
        let mut client = MqttClient {
            conn_addr: None,
            pub_addr: None,
            sub_addr: None,
            unsub_addr: None,
            stop_addr: None,
            disconnect_addr: None,
            client_name,
            options,
        };
        client.start_actors(reader, writer, message_recipient, error_recipient);
        client
    }

    /// Perform the connect operation to the remote MQTT server
    /// 
    /// Note: This function can only be called once for each client, calling it the second time will return an error
    pub fn connect(&mut self) -> Box<dyn Future<Item = (), Error = IoError>> {
        if let Some(connect_addr) = self.conn_addr.take() {
            let future = connect_addr
                .send(Connect {
                    user_name: self.options.user_name.take(),
                    password: self.options.password.take(),
                    keep_alive: self.options.keep_alive.take(),
                })
                .map_err(map_mailbox_error_to_io_error);
            Box::new(future)
        } else {
            Box::new(future::err(IoError::new(
                ErrorKind::AlreadyExists,
                "Already connected",
            )))
        }
    }

    /// Subscribe to the server with a topic and QoS
    pub fn subscribe(
        &self,
        topic: String,
        qos: QualityOfService,
    ) -> Box<dyn Future<Item = (), Error = IoError>> {
        if let Some(ref sub_addr) = self.sub_addr {
            let future = sub_addr
                .send(Subscribe::new(topic, qos))
                .map_err(map_mailbox_error_to_io_error);
            Box::new(future)
        } else {
            Box::new(future::err(address_not_found_error("subscribe")))
        }
    }

    /// Unsubscribe from the server
    pub fn unsubscribe(&self, topic: String) -> Box<dyn Future<Item = (), Error = IoError>> {
        if let Some(ref unsub_addr) = self.unsub_addr {
            let future = unsub_addr
                .send(Unsubscribe::new(topic))
                .map_err(map_mailbox_error_to_io_error);
            Box::new(future)
        } else {
            Box::new(future::err(address_not_found_error("unsubscribe")))
        }
    }

    /// Publish a message
    pub fn publish(
        &self,
        topic: String,
        qos: QualityOfService,
        payload: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = IoError>> {
        if let Some(ref pub_addr) = self.pub_addr {
            let future = pub_addr
                .send(Publish::new(topic, qos, payload))
                .map_err(map_mailbox_error_to_io_error);
            Box::new(future)
        } else {
            Box::new(future::err(address_not_found_error("publish")))
        }
    }

    /// Disconnect from the server
    pub fn disconnect(&self) -> Box<dyn Future<Item = (), Error = IoError>> {
        if let Some(ref disconnect_addr) = self.disconnect_addr {
            let future = disconnect_addr
                .send(Disconnect)
                .map_err(map_mailbox_error_to_io_error);
            Box::new(future)
        } else {
            Box::new(future::err(address_not_found_error("disconnect")))
        }
    }

    fn start_actors<TReader: AsyncRead + Send + 'static, TWriter: Write + Send + 'static>(
        &mut self,
        reader: TReader,
        writer: TWriter,
        publish_message_recipient: Recipient<PublishMessage>,
        error_recipient: Recipient<ErrorMessage>,
    ) {
        let send_recipient = SendActor::new(writer, error_recipient.clone())
            .start()
            .recipient();

        let stop_addr = StopActor::new().start();
        let stop_recipient = stop_addr.clone().recipient();
        let stop_recipient_container = stop_addr.clone().recipient();

        macro_rules! start_response_actor {
            ($addr_name:ident, $actor_name:ident, $status_recipient:ident) => {
                let $addr_name = $actor_name::new(
                    $status_recipient.clone(),
                    error_recipient.clone(),
                    stop_recipient.clone(),
                )
                .start();
                let _ = stop_recipient_container
                    .do_send(AddStopRecipient($addr_name.clone().recipient()));
            };
        }

        macro_rules! start_send_actor {
            ($addr_name:ident, $actor_name:ident, $status_recipient:ident) => {
                let $addr_name = $actor_name::new(
                    $status_recipient.clone(),
                    send_recipient.clone(),
                    error_recipient.clone(),
                    stop_recipient.clone(),
                )
                .start();
                let _ = stop_recipient_container
                    .do_send(AddStopRecipient($addr_name.clone().recipient()));
            };
        }

        macro_rules! start_status_actor {
            ($name:ident, $payload_type:ty) => {
                let $name = PacketStatusActor::<$payload_type>::new()
                    .start()
                    .recipient();
            };
        }

        start_status_actor!(publish_status_recipient, PublishPacketStatus);

        start_send_actor!(
            send_pub_actor_addr,
            SendPublishActor,
            publish_status_recipient
        );
        let recv_pub_actor_addr = RecvPublishActor::new(
            publish_status_recipient.clone(),
            send_recipient.clone(),
            error_recipient.clone(),
            stop_recipient.clone(),
            publish_message_recipient,
        )
        .start();
        let _ = stop_recipient_container
            .do_send(AddStopRecipient(recv_pub_actor_addr.clone().recipient()));
        start_response_actor!(puback_actor_addr, PubackActor, publish_status_recipient);
        start_send_actor!(pubrec_actor_addr, PubrecActor, publish_status_recipient);
        start_send_actor!(pubrel_actor_addr, PubrelActor, publish_status_recipient);
        start_response_actor!(pubcomp_actor_addr, PubcompActor, publish_status_recipient);

        start_status_actor!(ping_status_recipient, ());
        let send_ping_actor_addr = PingreqActor::new(
            ping_status_recipient.clone(),
            send_recipient.clone(),
            error_recipient.clone(),
            stop_recipient.clone(),
            PING_INTERVAL.clone(),
        )
        .start();
        let _ = stop_recipient_container
            .do_send(AddStopRecipient(send_ping_actor_addr.clone().recipient()));
        start_response_actor!(pingresp_actor_addr, PingrespActor, ping_status_recipient);

        start_status_actor!(subscribe_status_recipient, ());
        start_send_actor!(
            subscribe_actor_addr,
            SubscribeActor,
            subscribe_status_recipient
        );
        start_response_actor!(suback_actor_addr, SubackActor, subscribe_status_recipient);

        start_status_actor!(unsubscribe_status_recipient, ());
        start_send_actor!(
            unsubscribe_actor_addr,
            UnsubscribeActor,
            unsubscribe_status_recipient
        );
        start_response_actor!(
            unsuback_actor_addr,
            UnsubackActor,
            unsubscribe_status_recipient
        );

        let connect_status_actor_addr = PacketStatusActor::new().start();
        let connect_actor_addr = ConnectActor::new(
            send_recipient.clone(),
            connect_status_actor_addr.clone().recipient(),
            error_recipient.clone(),
            self.client_name.clone(),
        )
        .start();

        let connack_actor_addr = ConnackActor::new(
            connect_status_actor_addr.recipient(),
            error_recipient.clone(),
            connect_actor_addr.clone().recipient(),
            stop_recipient.clone(),
        )
        .start();

        let disconnect_actor_addr = DisconnectActor::new(
            send_recipient,
            error_recipient.clone(),
            stop_recipient.clone(),
        )
        .start();

        let dispatch_actor_addr = DispatchActor::new(
            error_recipient.clone(),
            stop_recipient,
            connack_actor_addr.recipient(),
            pingresp_actor_addr.recipient(),
            recv_pub_actor_addr.recipient(),
            puback_actor_addr.recipient(),
            pubrec_actor_addr.recipient(),
            pubrel_actor_addr.recipient(),
            pubcomp_actor_addr.recipient(),
            suback_actor_addr.recipient(),
            unsuback_actor_addr.recipient(),
        )
        .start();
        let recv_addr = RecvActor::new(
            reader,
            dispatch_actor_addr.clone().recipient(),
            error_recipient,
            stop_addr.clone().recipient(),
        )
        .start();
        let _ = stop_addr.do_send(AddStopRecipient(recv_addr.recipient()));

        self.sub_addr = Some(subscribe_actor_addr);
        self.unsub_addr = Some(unsubscribe_actor_addr);
        self.pub_addr = Some(send_pub_actor_addr);
        self.disconnect_addr = Some(disconnect_actor_addr);
        self.conn_addr = Some(connect_actor_addr);
        self.stop_addr = Some(stop_addr);
    }
}
