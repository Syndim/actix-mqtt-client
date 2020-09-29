use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::sync::Arc;

use actix::{Actor, Addr, MailboxError, Recipient};
use mqtt::QualityOfService;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::actors::actions::dispatch::DispatchActor;
use crate::actors::actions::recv::RecvActor;
use crate::actors::actions::send::SendActor;
use crate::actors::actions::status::{PacketStatusActor, StatusExistenceMessage};
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
use crate::actors::{ErrorMessage, StopMessage};
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
    conn_status_addr: Option<Addr<PacketStatusActor<()>>>,
    client_name: Arc<String>,
    options: Option<MqttOptions>,
}

impl MqttClient {
    /// Create a new MQTT client
    pub fn new<
        TReader: AsyncRead + Send + 'static + Unpin,
        TWriter: AsyncWrite + Send + 'static + Unpin,
    >(
        reader: TReader,
        writer: TWriter,
        client_name: String,
        options: MqttOptions,
        message_recipient: Recipient<PublishMessage>,
        error_recipient: Recipient<ErrorMessage>,
        stop_recipient: Option<Recipient<StopMessage>>,
    ) -> Self {
        let mut client = MqttClient {
            conn_addr: None,
            pub_addr: None,
            sub_addr: None,
            unsub_addr: None,
            stop_addr: None,
            disconnect_addr: None,
            conn_status_addr: None,
            client_name: Arc::new(client_name),
            options: Some(options),
        };
        client.start_actors(
            reader,
            writer,
            message_recipient,
            error_recipient,
            stop_recipient,
        );
        client
    }

    /// Returns the name of the client
    pub fn name(&self) -> &str {
        &*self.client_name
    }

    /// Perform the connect operation to the remote MQTT server
    ///
    /// Note: This function can only be called once for each client, calling it the second time will return an error
    /// Note: The successful return of this function *DOES NOT* mean that the MQTT connection is successful, if anything wrong happens the error actor will receive an error
    /// Note: Please use is_connected() to check whether the MQTT connection is successful or not
    pub async fn connect(&mut self) -> Result<(), IoError> {
        if let (Some(connect_addr), Some(mut options)) =
            (self.conn_addr.take(), self.options.take())
        {
            connect_addr
                .send(Connect {
                    user_name: options.user_name.take(),
                    password: options.password.take(),
                    keep_alive: options.keep_alive.take(),
                })
                .await
                .map_err(map_mailbox_error_to_io_error)
        } else {
            Err(IoError::new(ErrorKind::AlreadyExists, "Already connected"))
        }
    }

    /// Check whether the client has connected to the server successfully
    pub async fn is_connected(&self) -> IoResult<bool> {
        match self.conn_status_addr {
            Some(ref addr) => {
                let connected = addr
                    .send(StatusExistenceMessage(1))
                    .await
                    .map_err(|e| {
                        log::error!("Failed to get connection status: {}", e);
                        IoError::new(ErrorKind::NotConnected, "Failed to connect to server")
                    })?;
                Ok(connected)
            }
            None => Ok(false),
        }
    }

    /// Subscribe to the server with a topic and QoS
    pub async fn subscribe(&self, topic: String, qos: QualityOfService) -> Result<(), IoError> {
        if let Some(ref sub_addr) = self.sub_addr {
            sub_addr
                .send(Subscribe::new(topic, qos))
                .await
                .map_err(map_mailbox_error_to_io_error)
        } else {
            Err(address_not_found_error("subscribe"))
        }
    }

    /// Unsubscribe from the server
    pub async fn unsubscribe(&self, topic: String) -> Result<(), IoError> {
        if let Some(ref unsub_addr) = self.unsub_addr {
            unsub_addr
                .send(Unsubscribe::new(topic))
                .await
                .map_err(map_mailbox_error_to_io_error)
        } else {
            Err(address_not_found_error("unsubscribe"))
        }
    }

    /// Publish a message
    pub async fn publish(
        &self,
        topic: String,
        qos: QualityOfService,
        payload: Vec<u8>,
    ) -> Result<(), IoError> {
        if let Some(ref pub_addr) = self.pub_addr {
            pub_addr
                .send(Publish::new(topic, qos, payload))
                .await
                .map_err(map_mailbox_error_to_io_error)
        } else {
            Err(address_not_found_error("publish"))
        }
    }

    /// Disconnect from the server
    pub async fn disconnect(&mut self, force: bool) -> Result<(), IoError> {
        if let Some(ref disconnect_addr) = self.disconnect_addr {
            let result = disconnect_addr
                .send(Disconnect { force })
                .await
                .map_err(map_mailbox_error_to_io_error);
            self.clear_all_addrs(force);
            result
        } else {
            Err(address_not_found_error("disconnect"))
        }
    }

    /// Check if the client has been disconnected from the server, useful to check whether disconnection is completed
    pub fn is_disconnected(&self) -> bool {
        if let Some(ref disconnect_addr) = self.disconnect_addr {
            !disconnect_addr.connected()
        } else {
            true
        }
    }

    /// Set all addrs to None to prevent further operations
    fn clear_all_addrs(&mut self, include_disconnect: bool) {
        self.pub_addr = None;
        self.sub_addr = None;
        self.unsub_addr = None;
        self.conn_addr = None;
        self.conn_status_addr = None;

        if include_disconnect {
            self.disconnect_addr = None;
        }
    }

    fn start_actors<
        TReader: AsyncRead + Send + 'static + Unpin,
        TWriter: AsyncWrite + Send + 'static + Unpin,
    >(
        &mut self,
        reader: TReader,
        writer: TWriter,
        publish_message_recipient: Recipient<PublishMessage>,
        error_recipient: Recipient<ErrorMessage>,
        client_stop_recipient_option: Option<Recipient<StopMessage>>,
    ) {
        let stop_addr = StopActor::new().start();

        if let Some(client_stop_recipient) = client_stop_recipient_option {
            let _ = stop_addr.do_send(AddStopRecipient(client_stop_recipient));
        }

        let stop_recipient = stop_addr.clone().recipient();
        let stop_recipient_container = stop_addr.clone().recipient();

        let send_addr =
            SendActor::new(writer, error_recipient.clone(), stop_recipient.clone()).start();
        let send_recipient = send_addr.clone().recipient();
        let _ = stop_addr.do_send(AddStopRecipient(send_addr.recipient()));

        let disconnect_actor_addr = DisconnectActor::new(
            send_recipient.clone(),
            error_recipient.clone(),
            stop_recipient.clone(),
        )
        .start();

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
            ($name:ident, $status_name:tt, $payload_type:ty, $send_status_recipient:expr) => {
                let status_addr =
                    PacketStatusActor::<$payload_type>::new($status_name, $send_status_recipient)
                        .start();
                let $name = status_addr.clone().recipient();
                let _ = stop_recipient_container.do_send(AddStopRecipient(status_addr.recipient()));
            };
        }

        let send_status_recipient = disconnect_actor_addr.clone().recipient();
        start_status_actor!(
            publish_status_recipient,
            "Disconnect",
            PublishPacketStatus,
            Some(send_status_recipient)
        );

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

        start_status_actor!(subscribe_status_recipient, "Subscribe", (), None);
        start_send_actor!(
            subscribe_actor_addr,
            SubscribeActor,
            subscribe_status_recipient
        );
        start_response_actor!(suback_actor_addr, SubackActor, subscribe_status_recipient);

        start_status_actor!(unsubscribe_status_recipient, "Unsubscribe", (), None);
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

        let connect_status_actor_addr = PacketStatusActor::new("Connect", None).start();
        let connect_actor_addr = ConnectActor::new(
            send_recipient.clone(),
            connect_status_actor_addr.clone().recipient(),
            stop_recipient.clone(),
            error_recipient.clone(),
            (&*self.client_name).clone(),
        )
        .start();

        let connack_actor_addr = ConnackActor::new(
            connect_status_actor_addr.clone().recipient(),
            error_recipient.clone(),
            connect_actor_addr.clone().recipient(),
            stop_recipient.clone(),
        )
        .start();

        start_status_actor!(ping_status_recipient, "Ping", (), None);
        let send_ping_actor_addr = PingreqActor::new(
            ping_status_recipient.clone(),
            connect_status_actor_addr.clone().recipient(),
            send_recipient.clone(),
            error_recipient.clone(),
            stop_recipient.clone(),
            PING_INTERVAL.clone(),
        )
        .start();
        let _ = stop_recipient_container
            .do_send(AddStopRecipient(send_ping_actor_addr.clone().recipient()));
        start_response_actor!(pingresp_actor_addr, PingrespActor, ping_status_recipient);

        let dispatch_actor_addr = DispatchActor::new(
            error_recipient.clone(),
            stop_recipient.clone(),
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
            stop_recipient,
        )
        .start();
        let _ = stop_addr.do_send(AddStopRecipient(recv_addr.recipient()));

        self.sub_addr = Some(subscribe_actor_addr);
        self.unsub_addr = Some(unsubscribe_actor_addr);
        self.pub_addr = Some(send_pub_actor_addr);
        self.disconnect_addr = Some(disconnect_actor_addr);
        self.conn_addr = Some(connect_actor_addr);
        self.stop_addr = Some(stop_addr);
        self.conn_status_addr = Some(connect_status_actor_addr);
    }
}
