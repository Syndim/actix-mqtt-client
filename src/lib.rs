//! # A MQTT client based on actix framework
//!
//! The `actix-mqtt-client` crate is a mqtt client based on the [actix](https://github.com/actix/actix) framework
//!
//! ## Basic usage and example
//!
//! First, create 2 actix actors, one for receiving publish messages, the other one for receiving error messages from the client, you can also create an optional actix actor for receiving the stop message:
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
//! use std::io::Error as IoError;
//! use std::net::SocketAddr;
//! use std::str::FromStr;
//! use std::time::Duration;
//! use actix::{Actor, Arbiter, System};
//! use env_logger;
//! use tokio::io::split;
//! use tokio::net::TcpStream;
//! use tokio::time::{delay_until, Instant};
//! use actix_mqtt_client::client::{MqttClient, MqttOptions};
//!
//! System::run(|| {
//!     let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
//!     let future = async move {
//!         let result = async move {
//!             let stream = TcpStream::connect(socket_addr).await?;
//!             let (r, w) = split(stream);
//!             log::info!("TCP connected");
//!             let mut client = MqttClient::new(
//!                 r,
//!                 w,
//!                 String::from("test"),
//!                 MqttOptions::default(),
//!                 MessageActor.start().recipient(),
//!                 ErrorActor.start().recipient(),
//!                 None,
//!             );
//!             client.connect().await?;
//!             log::info!("MQTT connected");
//!             log::info!("Subscribe");
//!             client
//!                 .subscribe(String::from("test"), mqtt::QualityOfService::Level2)
//!                 .await?;
//!             log::info!("Publish");
//!             client
//!                 .publish(
//!                     String::from("test"),
//!                     mqtt::QualityOfService::Level0,
//!                     Vec::from("test".as_bytes()),
//!                 )
//!                 .await?;
//!             log::info!("Wait for 10s");
//!             let delay_time = Instant::now() + Duration::new(10, 0);
//!             delay_until(delay_time).await;
//!             client
//!                 .publish(
//!                     String::from("test"),
//!                     mqtt::QualityOfService::Level1,
//!                     Vec::from("test2".as_bytes()),
//!                 )
//!                 .await?;
//!             log::info!("Wait for 10s");
//!             let delay_time = Instant::now() + Duration::new(10, 0);
//!             delay_until(delay_time).await;
//!             client
//!                 .publish(
//!                     String::from("test"),
//!                     mqtt::QualityOfService::Level2,
//!                     Vec::from("test3".as_bytes()),
//!                 )
//!                 .await?;
//!             log::info!("Wait for 10s");
//!             let delay_time = Instant::now() + Duration::new(10, 0);
//!             delay_until(delay_time).await;
//!             log::info!("Disconnect");
//!             client.disconnect(false).await?;
//!             log::info!("Check if disconnect is successful");
//!             Ok(assert_eq!(true, client.is_disconnected())) as Result<(), IoError>
//!         }
//!         .await;
//!         result.unwrap()
//!     };
//!     Arbiter::spawn(future);
//! })
//! .unwrap();
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
pub use crate::actors::{ErrorMessage, StopMessage};
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
        use std::io::Error as IoError;
        use std::net::SocketAddr;
        use std::str::FromStr;
        use std::time::Duration;

        use actix::{Actor, Arbiter, System};
        use env_logger;
        use tokio::io::split;
        use tokio::net::TcpStream;
        use tokio::time::{delay_until, Instant};

        use crate::client::{MqttClient, MqttOptions};

        env_logger::init();

        System::run(|| {
            let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
            let future = async move {
                let result = async move {
                    let stream = TcpStream::connect(socket_addr).await?;
                    let (r, w) = split(stream);
                    log::info!("TCP connected");
                    let mut client = MqttClient::new(
                        r,
                        w,
                        String::from("test"),
                        MqttOptions::default(),
                        MessageActor.start().recipient(),
                        ErrorActor.start().recipient(),
                        None,
                    );
                    client.connect().await?;
                    log::info!("MQTT connected");
                    log::info!("Subscribe");
                    client
                        .subscribe(String::from("test"), mqtt::QualityOfService::Level2)
                        .await?;
                    log::info!("Publish");
                    client
                        .publish(
                            String::from("test"),
                            mqtt::QualityOfService::Level0,
                            Vec::from("test".as_bytes()),
                        )
                        .await?;
                    log::info!("Wait for 10s");
                    let delay_time = Instant::now() + Duration::new(10, 0);
                    delay_until(delay_time).await;
                    client
                        .publish(
                            String::from("test"),
                            mqtt::QualityOfService::Level1,
                            Vec::from("test2".as_bytes()),
                        )
                        .await?;
                    log::info!("Wait for 10s");
                    let delay_time = Instant::now() + Duration::new(10, 0);
                    delay_until(delay_time).await;
                    client
                        .publish(
                            String::from("test"),
                            mqtt::QualityOfService::Level2,
                            Vec::from("test3".as_bytes()),
                        )
                        .await?;
                    log::info!("Wait for 10s");
                    let delay_time = Instant::now() + Duration::new(10, 0);
                    delay_until(delay_time).await;
                    log::info!("Disconnect");
                    client.disconnect(false).await?;
                    log::info!("Check if disconnect is successful");
                    Ok(assert_eq!(true, client.is_disconnected())) as Result<(), IoError>
                }
                .await;
                result.unwrap()
            };
            Arbiter::spawn(future);
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
    fn test_random_publish_level0_cloned_client() {
        use std::io::Error as IoError;
        use std::net::SocketAddr;
        use std::str::FromStr;
        use std::time::Duration;

        use actix::{Actor, Arbiter, System};
        use env_logger;
        use futures::stream::StreamExt;
        use tokio::io::split;
        use tokio::net::TcpStream;
        use tokio::time::{delay_until, Instant};

        use crate::client::{MqttClient, MqttOptions};

        env_logger::init();

        let (sender, recv) = channel(100);
        System::run(|| {
            let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
            let future = async move {
                let result = async move {
                    let stream = TcpStream::connect(socket_addr).await?;
                    let (r, w) = split(stream);
                    let mut client = MqttClient::new(
                        r,
                        w,
                        String::from("test"),
                        MqttOptions::default(),
                        MessageActor(sender.clone()).start().recipient(),
                        ErrorActor.start().recipient(),
                        None,
                    );
                    log::info!("Connected");
                    client.connect().await?;
                    log::info!("Subscribe");
                    client
                        .subscribe(String::from("test"), mqtt::QualityOfService::Level0)
                        .await?;
                    async fn random_send(
                        client_id: i32,
                        client: MqttClient,
                        mut sender: Sender<(bool, Vec<u8>)>,
                    ) {
                        let mut count: i32 = 0;
                        loop {
                            count += 1;
                            use rand::RngCore;
                            let mut data = [0u8; 32];
                            rand::thread_rng().fill_bytes(&mut data);
                            let payload = Vec::from(&data[..]);
                            log::info!("[{}:{}] Publish {:?}", client_id, count, payload);
                            delay_until(Instant::now() + Duration::from_millis(10)).await;
                            sender.try_send((true, payload.clone())).unwrap();
                            client
                                .publish(
                                    String::from("test"),
                                    mqtt::QualityOfService::Level0,
                                    payload,
                                )
                                .await
                                .unwrap();
                        }
                    }

                    for i in 0..5 {
                        let client_clone = client.clone();
                        let sender_clone = sender.clone();
                        let future = random_send(i, client_clone, sender_clone);
                        Arbiter::spawn(future);
                    }

                    Ok(()) as Result<(), IoError>
                }
                .await;
                result.unwrap();
            };

            Arbiter::spawn(future);
            let recv_future = async {
                let result = async {
                    recv.fold((), |_, (is_send, payload)| async move {
                        let mut p = PACKETS.lock().unwrap();
                        if is_send {
                            p.insert(payload);
                        } else if p.contains(&payload) {
                            p.remove(&payload);
                        }

                        log::info!("Pending recv items: {}", p.len());

                        ()
                    })
                    .await;
                    Ok(()) as Result<(), IoError>
                }
                .await;
                result.unwrap()
            };
            Arbiter::spawn(recv_future);
        })
        .unwrap();
    }

    #[test]
    fn test_random_publish_level0_created_client() {
        use std::io::Error as IoError;
        use std::net::SocketAddr;
        use std::str::FromStr;
        use std::time::Duration;

        use actix::{Actor, Arbiter, System};
        use env_logger;
        use futures::stream::StreamExt;
        use tokio::io::split;
        use tokio::net::TcpStream;
        use tokio::time::{delay_until, Instant};

        use crate::client::{MqttClient, MqttOptions};

        env_logger::init();

        async fn test_send(client_id: i32, sender: Sender<(bool, Vec<u8>)>) {
            let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
            async move {
                let stream = TcpStream::connect(socket_addr).await?;
                let (r, w) = split(stream);
                let mut client = MqttClient::new(
                    r,
                    w,
                    format!("test_{}", client_id),
                    MqttOptions::default(),
                    MessageActor(sender.clone()).start().recipient(),
                    ErrorActor.start().recipient(),
                    None,
                );
                log::info!("Connected");
                client.connect().await?;
                log::info!("Subscribe");
                client
                    .subscribe(String::from("test"), mqtt::QualityOfService::Level0)
                    .await?;
                async fn random_send(
                    client_id: i32,
                    client: MqttClient,
                    mut sender: Sender<(bool, Vec<u8>)>,
                ) {
                    let mut count: i32 = 0;
                    loop {
                        count += 1;
                        use rand::RngCore;
                        let mut data = [0u8; 32];
                        rand::thread_rng().fill_bytes(&mut data);
                        let payload = Vec::from(&data[..]);
                        log::info!("[{}:{}] Publish {:?}", client_id, count, payload);
                        delay_until(Instant::now() + Duration::from_millis(10)).await;
                        sender.try_send((true, payload.clone())).unwrap();
                        client
                            .publish(
                                String::from("test"),
                                mqtt::QualityOfService::Level0,
                                payload,
                            )
                            .await
                            .unwrap();
                    }
                }

                let future = random_send(client_id, client, sender);
                Arbiter::spawn(future);

                Ok(()) as Result<(), IoError>
            }
            .await
            .unwrap();
        }

        System::run(|| {
            let (sender, recv) = channel(100);
            for i in 0..5 {
                let future = test_send(i, sender.clone());
                Arbiter::spawn(future);
            }

            let recv_future = async {
                let result = async {
                    recv.fold((), |_, (is_send, payload)| async move {
                        let mut p = PACKETS.lock().unwrap();
                        if is_send {
                            p.insert(payload);
                        } else if p.contains(&payload) {
                            p.remove(&payload);
                        }

                        log::info!("Pending recv items: {}", p.len());

                        ()
                    })
                    .await;
                    Ok(()) as Result<(), IoError>
                }
                .await;
                result.unwrap()
            };
            Arbiter::spawn(recv_future);
        })
        .unwrap();
    }

    #[test]
    fn test_random_publish_level2() {
        use std::io::Error as IoError;
        use std::net::SocketAddr;
        use std::str::FromStr;
        use std::time::Duration;

        use actix::{Actor, Arbiter, System};
        use env_logger;
        use futures::stream::StreamExt;
        use tokio::io::split;
        use tokio::net::TcpStream;
        use tokio::time::{delay_until, Instant};

        use crate::client::{MqttClient, MqttOptions};

        env_logger::init();

        let (sender, recv) = channel(100);
        let sender_clone = sender.clone();

        System::run(|| {
            let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
            let future = async move {
                let result = async move {
                    let stream = TcpStream::connect(socket_addr).await?;
                    let (r, w) = split(stream);
                    let mut client = MqttClient::new(
                        r,
                        w,
                        String::from("test"),
                        MqttOptions::default(),
                        MessageActor(sender).start().recipient(),
                        ErrorActor.start().recipient(),
                        None,
                    );
                    log::info!("Connected");
                    client.connect().await?;
                    log::info!("Subscribe");
                    client
                        .subscribe(String::from("test"), mqtt::QualityOfService::Level2)
                        .await?;
                    futures::stream::repeat(())
                        .fold((client, sender_clone), |(client, mut sender), _| async {
                            use rand::RngCore;
                            let mut data = [0u8; 32];
                            rand::thread_rng().fill_bytes(&mut data);
                            let payload = Vec::from(&data[..]);
                            log::info!("Publish {:?}", payload);
                            delay_until(Instant::now() + Duration::from_millis(10)).await;
                            sender.try_send((true, payload.clone())).unwrap();
                            client
                                .publish(
                                    String::from("test"),
                                    mqtt::QualityOfService::Level2,
                                    payload,
                                )
                                .await
                                .unwrap();
                            (client, sender)
                        })
                        .await;
                    Ok(()) as Result<(), IoError>
                }
                .await;
                result.unwrap()
            };
            Arbiter::spawn(future);
            let recv_future = async {
                let result = async {
                    recv.fold((), |_, (is_send, payload)| async move {
                        let mut p = PACKETS.lock().unwrap();
                        if is_send {
                            p.insert(payload);
                        } else if !p.contains(&payload) {
                            panic!("Multiple receive for level 2: {:?}", payload);
                        } else {
                            p.remove(&payload);
                        }

                        log::info!("Pending recv items: {}", p.len());

                        ()
                    })
                    .await;
                    Ok(()) as Result<(), IoError>
                }
                .await;
                result.unwrap()
            };
            Arbiter::spawn(recv_future);
        })
        .unwrap();
    }
}
