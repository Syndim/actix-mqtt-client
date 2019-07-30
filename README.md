# A MQTT v3.1.1 client based on actix framework

The `actix-mqtt-client` crate is a mqtt v3.1.1 client based on the [actix](https://github.com/actix/actix) framework

[![Build Status](https://travis-ci.org/Syndim/actix-mqtt-client.svg?branch=master)](https://travis-ci.org/Syndim/actix-mqtt-client) [![crates.io](https://img.shields.io/crates/v/actix-mqtt-client.svg)](https://crates.io/crates/actix-mqtt-client) [![docs.rs](https://docs.rs/actix-mqtt-client/badge.svg)](https://docs.rs/actix-mqtt-client)

## Basic usage and example

First, create 2 actix actors, one for receiving publish messages, the other one for receiving error messages from the client:
```rust
pub struct ErrorActor;

impl actix::Actor for ErrorActor {
    type Context = actix::Context<Self>;
}

impl actix::Handler<ErrorMessage> for ErrorActor {
    type Result = ();
    fn handle(&mut self, error: ErrorMessage, _: &mut Self::Context) -> Self::Result {
        log::error!("{}", error.0);
    }
}

pub struct MessageActor;

impl actix::Actor for MessageActor {
    type Context = actix::Context<Self>;
}

impl actix::Handler<PublishMessage> for MessageActor {
    type Result = ();
    fn handle(
        &mut self,
        msg: PublishMessage,
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
```

Then, connect to the server(using tokio) and use the read and write part of the stream along with the actors to create a MqttClient:
```rust
System::run(|| {
    let socket_addr = SocketAddr::from_str("127.0.0.1:1883").unwrap();
    Arbiter::spawn(
        TcpStream::connect(&socket_addr)
            .and_then(|stream| {
                let (r, w) = stream.split();
                let mut client = MqttClient::new(
                    r,
                    w,
                    String::from("mqtt_client"),
                    MqttOptions::default(),
                    MessageActor.start().recipient(),
                    ErrorActor.start().recipient(),
                );
                log::info!("Connect");
                client.connect().map(|_| client)
            })
            .and_then(|client| {
                log::info!("Subscribe");
                client
                    .subscribe(String::from("topic"), QualityOfService::Level2)
                    .map(|_| client)
            })
            .and_then(|client| {
                log::info!("Publish Level0");
                client
                    .publish(
                        String::from("topic"),
                        QualityOfService::Level0,
                        Vec::from("level0".as_bytes()),
                    )
                    .map(|_| client)
            })
            .and_then(|client| {
                log::info!("Publish Level1");
                client
                    .publish(
                        String::from("topic"),
                        QualityOfService::Level1,
                        Vec::from("level1".as_bytes()),
                    )
                    .map(|_| client)
            })
            .and_then(|client| {
                log::info!("Publish Level2");
                client
                    .publish(
                        String::from("topic"),
                        QualityOfService::Level2,
                        Vec::from("level2".as_bytes()),
                    )
                    .map(|_| client)
            })
            .and_then(|client| {
                log::info!("Disconnect");
                client.disconnect()
            })
            .map_err(|_| ()),
    );
})
.unwrap();
```
