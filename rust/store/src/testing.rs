//! Captures the messages of the events that code under test emits, so a
//! test can check that a trace shows what happened.

use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

use tracing::Event;
use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::instrument::WithSubscriber as _;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt as _;

#[derive(Clone, Default)]
struct Messages(Arc<Mutex<Vec<String>>>);

impl<S: Subscriber> Layer<S> for Messages {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        struct Message(String);
        impl Visit for Message {
            fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        if let Ok(mut messages) = self.0.lock() {
            messages.push(message.0);
        }
    }
}

/// Runs `future` and returns what it returned, with the message of each
/// event it emitted.
pub(crate) async fn messages_of<F: Future>(future: F) -> (F::Output, Vec<String>) {
    let messages = Messages::default();
    let subscriber = tracing_subscriber::registry().with(messages.clone());
    let output = future.with_subscriber(subscriber).await;
    let messages = messages.0.lock().map(|m| m.clone()).unwrap_or_default();
    (output, messages)
}
