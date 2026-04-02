use crate::app::ports::{AuthBusinessEvent, Observability};

/// Observability adapter that intentionally drops all emitted events.
pub struct NoopObservability;

impl Observability for NoopObservability {
    fn emit(&self, _event: AuthBusinessEvent) {}
}
