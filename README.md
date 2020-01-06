# A MQTT v3.1.1 client based on actix framework

The `actix-mqtt-client` crate is a mqtt v3.1.1 client based on the [actix](https://github.com/actix/actix) framework

[![Build Status](https://travis-ci.org/Syndim/actix-mqtt-client.svg?branch=master)](https://travis-ci.org/Syndim/actix-mqtt-client) [![crates.io](https://img.shields.io/crates/v/actix-mqtt-client.svg)](https://crates.io/crates/actix-mqtt-client) [![docs.rs](https://docs.rs/actix-mqtt-client/badge.svg)](https://docs.rs/actix-mqtt-client)

## Basic usage and example

First, create 2 actix actors, one for receiving publish messages, the other one for receiving error messages from the client, you can also create an optional actix actor for receiving the stop message:
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
```
