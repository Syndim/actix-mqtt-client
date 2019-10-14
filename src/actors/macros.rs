macro_rules! impl_empty_actor {
    ($name:ident) => {
        impl_empty_actor!($name, crate::consts::DEFAULT_MAILBOX_CAPACITY);
    };
    ($name:ident, $mailbox_capacity:expr) => {
        impl actix::Actor for $name {
            type Context = actix::Context<Self>;
            fn started(&mut self, ctx: &mut Self::Context) {
                ctx.set_mailbox_capacity($mailbox_capacity);
                log::info!(concat!(stringify!($name), " started"));
            }

            fn stopped(&mut self, _: &mut Self::Context) {
                log::info!(concat!(stringify!($name), " stopped"));
            }
        }
    };
}

macro_rules! impl_generic_empty_actor {
    ($name:ident) => {
        impl_generic_empty_actor!($name, crate::consts::DEFAULT_MAILBOX_CAPACITY);
    };
    ($name:ident, $mailbox_capacity:expr) => {
        impl<T: 'static> actix::Actor for $name<T> {
            type Context = actix::Context<Self>;
            fn started(&mut self, ctx: &mut Self::Context) {
                ctx.set_mailbox_capacity($mailbox_capacity);
                log::info!(concat!(stringify!($name), " started"));
            }

            fn stopped(&mut self, _: &mut Self::Context) {
                log::info!(concat!(stringify!($name), " stopped"));
            }
        }
    };
}

macro_rules! impl_stop_handler {
    ($name:ident) => {
        impl actix::Handler<crate::actors::StopMessage> for $name {
            type Result = ();

            fn handle(
                &mut self,
                _: crate::actors::StopMessage,
                ctx: &mut Self::Context,
            ) -> Self::Result {
                log::info!(concat!("Handle stop message for ", stringify!($name)));
                use actix::ActorContext;
                ctx.stop();
            }
        }
    };
}

macro_rules! impl_generic_stop_handler {
    ($name:ident) => {
        impl<T: 'static> actix::Handler<crate::actors::StopMessage> for $name<T> {
            type Result = ();

            fn handle(
                &mut self,
                _: crate::actors::StopMessage,
                ctx: &mut Self::Context,
            ) -> Self::Result {
                log::info!(concat!("Handle stop message for ", stringify!($name)));
                use actix::ActorContext;
                ctx.stop();
            }
        }
    };
}
