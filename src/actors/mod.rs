use std::io;

#[macro_use]
pub mod macros;
pub mod actions;
pub mod packets;
mod utils;

use std::io::ErrorKind;
use std::thread::sleep;

use actix::dev::ToEnvelope;
use actix::prelude::SendError;
use actix::{Actor, Addr, Arbiter, Handler, MailboxError, Message, Recipient, System};
use log::{error, info};
use tokio::time::{delay_until, Instant};

use crate::consts::{DELAY_BEFORE_SHUTDOWN, RESEND_DELAY};
use crate::errors;

/// The actix message indicating that the client is about to stop
#[derive(Message)]
#[rtype(result = "()")]
pub struct StopMessage;

/// The actix message containing the error happens inside the client
#[derive(Message)]
#[rtype(result = "()")]
pub struct ErrorMessage(pub io::Error);

pub fn stop_system(stop_recipient: &Recipient<StopMessage>, code: i32) {
    // Can't recover, exit the system
    error!("Stopping the system");
    let _ = stop_recipient.do_send(StopMessage);
    sleep(DELAY_BEFORE_SHUTDOWN.clone());
    System::current().stop_with_code(code);
}

pub fn force_stop_system(code: i32) {
    error!("Force stopping the system");
    sleep(DELAY_BEFORE_SHUTDOWN.clone());
    System::current().stop_with_code(code);
}

pub fn send_error<T: AsRef<str>>(
    error_recipient: &Recipient<ErrorMessage>,
    kind: io::ErrorKind,
    message: T,
) {
    let error = io::Error::new(kind, message.as_ref());
    let _ = error_recipient.do_send(ErrorMessage(error));
}

fn resend<TActor, TMessage>(addr: Addr<TActor>, msg: TMessage)
where
    TMessage: Message + Send + 'static,
    TActor: Actor + Handler<TMessage>,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
{
    info!("Schedule resend message");
    let later_func = async move {
        let delay_time = Instant::now() + RESEND_DELAY.clone();
        delay_until(delay_time).await;
        addr.do_send(msg);
    };
    Arbiter::spawn(later_func);
}

fn handle_send_error<T>(
    tag: &str,
    e: SendError<T>,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
) {
    match e {
        SendError::Closed(_) => {
            send_error(
                error_recipient,
                ErrorKind::Interrupted,
                format!("[{}] Target mailbox is closed", tag),
            );
            stop_system(stop_recipient, errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
        }
        SendError::Full(_) => {
            // Do nothing
            send_error(error_recipient, ErrorKind::Other, format!("[{}] Target mailbox is full", tag));
        }
    }
}

fn handle_send_error_with_resend<T, TActor, TMessage>(
    tag: &str,
    e: SendError<T>,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
    addr: Addr<TActor>,
    msg: TMessage,
) where
    TMessage: Message + Send + 'static,
    TActor: Actor + Handler<TMessage>,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
{
    match e {
        SendError::Closed(_) => {
            send_error(
                error_recipient,
                ErrorKind::Interrupted,
                format!("[{}] Target mailbox is closed", tag),
            );
            stop_system(stop_recipient, errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
        }
        SendError::Full(_) => {
            send_error(
                error_recipient,
                ErrorKind::Other,
                format!("[{}] Target mailbox is full, will retry send", tag),
            );
            resend(addr, msg);
        }
    }
}

// fn handle_mailbox_error(
//     e: MailboxError,
//     error_recipient: &Recipient<ErrorMessage>,
//     stop_recipient: &Recipient<StopMessage>,
// ) {
//     match e {
//         MailboxError::Closed => {
//             send_error(
//                 error_recipient,
//                 ErrorKind::Interrupted,
//                 "Target mailbox is closed",
//             );
//             stop_system(stop_recipient, errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
//         }
//         MailboxError::Timeout => {
//             // Do nothing
//             send_error(error_recipient, ErrorKind::Other, "Send timeout");
//         }
//     }
// }

fn handle_mailbox_error_with_resend<TActor, TMessage>(
    e: MailboxError,
    error_recipient: &Recipient<ErrorMessage>,
    stop_recipient: &Recipient<StopMessage>,
    addr: Addr<TActor>,
    msg: TMessage,
) where
    TMessage: Message + Send + 'static,
    TActor: Actor + Handler<TMessage>,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
{
    match e {
        MailboxError::Closed => {
            send_error(
                error_recipient,
                ErrorKind::Interrupted,
                "Target mailbox is closed",
            );
            stop_system(stop_recipient, errors::ERROR_CODE_FAILED_TO_SEND_MESSAGE);
        }
        MailboxError::Timeout => {
            send_error(
                error_recipient,
                ErrorKind::Other,
                "Send timeout, will resend",
            );

            resend(addr, msg);
        }
    }
}
