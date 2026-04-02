use crate::app::ports::{AuthBusinessEvent, Observability};

/// Local observability adapter that prints structured auth events.
pub struct MockObservability;

impl Observability for MockObservability {
    fn emit(&self, event: AuthBusinessEvent) {
        println!("auth_event={:?}", event);
    }
}
