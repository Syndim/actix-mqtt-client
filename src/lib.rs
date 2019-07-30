//! # A MQTT client based on actix framework
//!
//! The `actix-mqtt-client` crate is a mqtt client based on the [actix](https://github.com/actix/actix) framework
//!
//! ## Basic usage and example
//!
//! First, create 2 actix actors, one for receiving publish messages, the other one for receiving error messages from the client:
//! ```rust
//! pub struct ErrorActor;
//!
//! impl actix::Actor for ErrorActor {
//!     type Context = actix::Context<Self>;
//! }
//!
//! impl actix::Handler<ErrorMessage> for ErrorActor {
//!     type Result = ();
//!     fn handle(&mut self, error: ErrorMessage, _: &mut Self::Context) -> Self::Result {
//!         log::error!("{}", error.0);
//!     }
//! }
//!
//! pub struct MessageActor;
//!
//! impl actix::Actor for MessageActor {
//!     type Context = actix::Context<Self>;
//! }
//!
//! impl actix::Handler<PublishMessage> for MessageActor {
//!     type Result = ();
//!     fn handle(
//!         &mut self,
//!         msg: PublishMessage,
//!         _: &mut Self::Context,
//!     ) -> Self::Result {
//!         log::info!(
//!             "Got message: id:{}, topic: {}, payload: {:?}",
//!             msg.id,
//!             msg.topic_name,
//!             msg.payload
//!         );
//!     }
//! }
//! ```
//!
//! Then, connect to the server(using tokio) and use the read and write part of the stream along with the actors to create a [MqttClient](struct.MqttClient.html):
//! ```rust
//! System::run(|| {
//!         let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
//!         Arbiter::spawn(
//!             TcpStream::connect(&socket_addr)
//!                 .and_then(|stream| {
//!                     let (r, w) = stream.split();
//!                     let mut client = MqttClient::new(
//!                         r,
//!                         w,
//!                         String::from("mqtt_client"),
//!                         MqttOptions::default(),
//!                         MessageActor.start().recipient(),
//!                         ErrorActor.start().recipient(),
//!                     );
//!                     log::info!("Connect");
//!                     client.connect().map(|_| client)
//!                 })
//!                 .and_then(|client| {
//!                     log::info!("Subscribe");
//!                     client
//!                         .subscribe(String::from("topic"), QualityOfService::Level2)
//!                         .map(|_| client)
//!                 })
//!                 .and_then(|client| {
//!                     log::info!("Publish Level0");
//!                     client
//!                         .publish(
//!                             String::from("topic"),
//!                             QualityOfService::Level0,
//!                             Vec::from("level0".as_bytes()),
//!                         )
//!                         .map(|_| client)
//!                 })
//!                 .and_then(|client| {
//!                     log::info!("Publish Level1");
//!                     client
//!                         .publish(
//!                             String::from("topic"),
//!                             QualityOfService::Level1,
//!                             Vec::from("level1".as_bytes()),
//!                         )
//!                         .map(|_| client)
//!                 })
//!                 .and_then(|client| {
//!                     log::info!("Publish Level2");
//!                     client
//!                         .publish(
//!                             String::from("topic"),
//!                             QualityOfService::Level2,
//!                             Vec::from("level2".as_bytes()),
//!                         )
//!                         .map(|_| client)
//!                 })
//!                 .and_then(|client| {
//!                     log::info!("Disconnect");
//!                     client.disconnect()
//!                 })
//!                 .map_err(|_| ()),
//!         );
//!     })
//!     .unwrap();
//! ```

mod actors;
mod client;
mod consts;
mod errors;

pub use actix;
pub use futures;
pub use mqtt::QualityOfService;
pub use tokio;

pub use crate::actors::packets::PublishMessage;
pub use crate::actors::ErrorMessage;
pub use crate::client::{MqttClient, MqttOptions};

#[cfg(test)]
mod tests {
    pub struct ErrorActor;

    impl actix::Actor for ErrorActor {
        type Context = actix::Context<Self>;
    }

    impl actix::Handler<super::ErrorMessage> for ErrorActor {
        type Result = ();
        fn handle(&mut self, error: super::ErrorMessage, _: &mut Self::Context) -> Self::Result {
            log::error!("{}", error.0);
        }
    }

    pub struct MessageActor;

    impl actix::Actor for MessageActor {
        type Context = actix::Context<Self>;
    }

    impl actix::Handler<crate::actors::packets::PublishMessage> for MessageActor {
        type Result = ();
        fn handle(
            &mut self,
            msg: crate::actors::packets::PublishMessage,
            _: &mut Self::Context,
        ) -> Self::Result {
            log::info!(
                "Got message: id:{}, topic: {}, payload: {:?}",
                msg.id,
                msg.topic_name,
                msg.payload
            );
        }
    }

    #[test]
    fn test_client() {
        use std::io::{Error as IoError, ErrorKind};
        use std::net::SocketAddr;
        use std::str::FromStr;
        use std::time::{Duration, Instant};

        use actix::{Actor, Arbiter, System};
        use env_logger;
        use futures::Future;
        use tokio::io::AsyncRead;
        use tokio::net::TcpStream;
        use tokio::timer::Delay;

        use crate::client::{MqttClient, MqttOptions};

        env_logger::init();

        System::run(|| {
            let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
            Arbiter::spawn(
                TcpStream::connect(&socket_addr)
                    .and_then(|stream| {
                        let (r, w) = stream.split();
                        let mut client = MqttClient::new(
                            r,
                            w,
                            String::from("test"),
                            MqttOptions::default(),
                            MessageActor.start().recipient(),
                            ErrorActor.start().recipient(),
                        );
                        log::info!("Connected");
                        client.connect().map(|_| client)
                    })
                    .and_then(|client| {
                        log::info!("Subscribe");
                        client
                            .subscribe(String::from("test"), mqtt::QualityOfService::Level2)
                            .map(|_| client)
                    })
                    .and_then(|client| {
                        log::info!("Publish");
                        client
                            .publish(
                                String::from("test"),
                                mqtt::QualityOfService::Level0,
                                Vec::from("test".as_bytes()),
                            )
                            .map(|_| client)
                    })
                    .and_then(|client| {
                        log::info!("Wait for 10s");
                        let delay_time = Instant::now() + Duration::new(10, 0);
                        Delay::new(delay_time)
                            .map(|_| client)
                            .map_err(|e| IoError::new(ErrorKind::Other, format!("{}", e)))
                    })
                    .and_then(|client| {
                        client
                            .publish(
                                String::from("test"),
                                mqtt::QualityOfService::Level1,
                                Vec::from("test2".as_bytes()),
                            )
                            .map(|_| client)
                    })
                    .and_then(|client| {
                        log::info!("Wait for 10s");
                        let delay_time = Instant::now() + Duration::new(10, 0);
                        Delay::new(delay_time)
                            .map(|_| client)
                            .map_err(|e| IoError::new(ErrorKind::Other, format!("{}", e)))
                    })
                    .and_then(|client| {
                        client
                            .publish(
                                String::from("test"),
                                mqtt::QualityOfService::Level2,
                                Vec::from("test3".as_bytes()),
                            )
                            .map(|_| client)
                    })
                    .and_then(|client| {
                        log::info!("Wait for 10s");
                        let delay_time = Instant::now() + Duration::new(10, 0);
                        Delay::new(delay_time)
                            .map(|_| client)
                            .map_err(|e| IoError::new(ErrorKind::Other, format!("{}", e)))
                    })
                    .and_then(|client| {
                        log::info!("Disconnect");
                        client.disconnect()
                    })
                    .map_err(|_| ()),
            );
        })
        .unwrap();
    }
}

#[cfg(test)]
mod random_test {
    use tokio::sync::mpsc::{channel, Sender};

    pub struct ErrorActor;

    impl actix::Actor for ErrorActor {
        type Context = actix::Context<Self>;
    }

    impl actix::Handler<super::ErrorMessage> for ErrorActor {
        type Result = ();
        fn handle(&mut self, error: super::ErrorMessage, _: &mut Self::Context) -> Self::Result {
            log::error!("{}", error.0);
        }
    }

    pub struct MessageActor(Sender<(bool, Vec<u8>)>);

    impl actix::Actor for MessageActor {
        type Context = actix::Context<Self>;
    }

    impl actix::Handler<crate::actors::packets::PublishMessage> for MessageActor {
        type Result = ();
        fn handle(
            &mut self,
            msg: crate::actors::packets::PublishMessage,
            _: &mut Self::Context,
        ) -> Self::Result {
            log::info!(
                "Got message: id:{}, topic: {}, payload: {:?}",
                msg.id,
                msg.topic_name,
                msg.payload
            );

            self.0.try_send((false, msg.payload)).unwrap();
        }
    }

    lazy_static::lazy_static! {
        static ref PACKETS: std::sync::Mutex<std::collections::HashSet<Vec<u8>>> = std::sync::Mutex::new(std::collections::HashSet::new());
    }

    #[test]
    fn test_random_publish_level2() {
        use std::net::SocketAddr;
        use std::str::FromStr;

        use actix::{Actor, Arbiter, System};
        use env_logger;
        use futures::{Future, Stream};
        use tokio::io::AsyncRead;
        use tokio::net::TcpStream;

        use crate::client::{MqttClient, MqttOptions};

        env_logger::init();

        let (sender, recv) = channel(100);
        let sender_clone = sender.clone();

        System::run(|| {
            let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
            Arbiter::spawn(
                TcpStream::connect(&socket_addr)
                    .and_then(|stream| {
                        let (r, w) = stream.split();
                        let mut client = MqttClient::new(
                            r,
                            w,
                            String::from("test"),
                            MqttOptions::default(),
                            MessageActor(sender).start().recipient(),
                            ErrorActor.start().recipient(),
                        );
                        log::info!("Connected");
                        client.connect().map(|_| client)
                    })
                    .and_then(|client| {
                        log::info!("Subscribe");
                        client
                            .subscribe(String::from("test"), mqtt::QualityOfService::Level2)
                            .map(|_| client)
                    })
                    .and_then(move |client| {
                        futures::stream::repeat(()).fold(
                            (client, sender_clone),
                            |(client, mut sender), _| {
                                use rand::RngCore;
                                let mut data = [0u8; 32];
                                rand::thread_rng().fill_bytes(&mut data);
                                let payload = Vec::from(&data[..]);
                                log::info!("Publish {:?}", payload);
                                sender.try_send((true, payload.clone())).unwrap();
                                client
                                    .publish(
                                        String::from("test"),
                                        mqtt::QualityOfService::Level2,
                                        payload,
                                    )
                                    .map(|_| (client, sender))
                            },
                        )
                    })
                    .map(|_| ())
                    .map_err(|_| ()),
            );
            Arbiter::spawn(
                recv.fold((), |_, (is_send, payload)| {
                    let mut p = PACKETS.lock().unwrap();
                    if is_send {
                        p.insert(payload);
                    } else if !p.contains(&payload) {
                        panic!("Multiple receive for level 2: {:?}", payload);
                    } else {
                        p.remove(&payload);
                    }

                    log::info!("Pending recv items: {}", p.len());

                    futures::future::ok(())
                })
                .map(|_| ())
                .map_err(|_| ()),
            );
        })
        .unwrap();
    }
}
