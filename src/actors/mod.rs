use std::io;

#[macro_use]
pub mod macros;
pub mod actions;
pub mod packets;
mod utils;

use std::io::ErrorKind;

use actix::dev::ToEnvelope;
use actix::prelude::SendError;
use actix::{Actor, Addr, Arbiter, Handler, MailboxError, Message, Recipient};
use log::trace;
use tokio::time::{delay_until, Instant};

use crate::consts::RESEND_DELAY;

/// The actix message indicating that the client is about to stop
#[derive(Message)]
#[rtype(result = "()")]
pub struct StopMessage;

/// The actix message containing the error happens inside the client
#[derive(Message)]
#[rtype(result = "()")]
pub struct ErrorMessage(pub io::Error);

pub fn send_error<T: AsRef<str>>(
    tag: &str,
    error_recipient: &Recipient<ErrorMessage>,
    kind: io::ErrorKind,
    message: T,
) {
    let error = io::Error::new(kind, message.as_ref());
    let send_result = error_recipient.try_send(ErrorMessage(error));
    log::debug!(
        "[{}] Send result for error recipient: {:?}",
        tag,
        send_result
    );
}

fn resend<TActor, TMessage>(addr: Addr<TActor>, msg: TMessage)
where
    TMessage: Message + Send + 'static,
    TActor: Actor + Handler<TMessage>,
    TMessage::Result: Send,
    TActor::Context: ToEnvelope<TActor, TMessage>,
{
    trace!("Schedule resend message");
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
                tag,
                error_recipient,
                ErrorKind::Interrupted,
                format!("[{}] Target mailbox is closed", tag),
            );
            let _ = stop_recipient.do_send(StopMessage);
        }
        SendError::Full(_) => {
            // Do nothing
            send_error(
                tag,
                error_recipient,
                ErrorKind::Other,
                format!("[{}] Target mailbox is full", tag),
            );
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
                tag,
                error_recipient,
                ErrorKind::Interrupted,
                format!("[{}] Target mailbox is closed", tag),
            );
            let _ = stop_recipient.do_send(StopMessage);
        }
        SendError::Full(_) => {
            send_error(
                tag,
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
    tag: &str,
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
                tag,
                error_recipient,
                ErrorKind::Interrupted,
                "Target mailbox is closed",
            );
            let _ = stop_recipient.do_send(StopMessage);
        }
        MailboxError::Timeout => {
            send_error(
                tag,
                error_recipient,
                ErrorKind::Other,
                "Send timeout, will resend",
            );

            resend(addr, msg);
        }
    }
}
