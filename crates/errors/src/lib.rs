//! Error-reporting domain: Sentry envelope decoding, event normalization, and
//! issue fingerprinting. HTTP and storage live in `apps/server`.

mod envelope;
mod event;

pub use envelope::{BodyError, Envelope, EnvelopeError, EnvelopeItem, decode_body, parse_envelope};
pub use event::{NormalizedEvent, normalize_event};
