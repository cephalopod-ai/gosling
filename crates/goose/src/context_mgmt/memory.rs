//! Seam for feeding retrieved memory into the Context Manager.
//!
//! The `RetrievedMemory` slot in a [`super::packet::ContextPacket`] is fed
//! from a [`MemorySource`]. This MVP ships only [`NoopMemorySource`], so the
//! slot stays empty and behavior is unchanged — but the retrieval interface,
//! budget enforcement, and packet placement are real and tested, so a
//! persistent memory backend only has to implement one trait.

use crate::conversation::message::Message;

/// One unit of recalled context, ready to be rendered into the packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryItem {
    pub content: String,
    /// Where this memory came from (e.g. "session:abc123", "note"), shown
    /// alongside the content so the model can weigh its provenance.
    pub source: String,
}

/// What a memory backend gets to look at when deciding what to recall.
#[derive(Debug, Clone)]
pub struct MemoryQuery<'a> {
    pub session_id: &'a str,
    /// The conversation about to be sent, most recent last. Backends will
    /// typically key retrieval off the trailing user message.
    pub messages: &'a [Message],
    /// Hard token ceiling for the slot; returning more than fits is fine —
    /// the Context Manager enforces the budget and records the overflow.
    pub reserved_tokens: usize,
}

/// A source of retrieved memory. Implementations must be cheap and
/// infallible from the caller's perspective: this runs on the hot path in
/// front of every provider call, so blocking I/O or fallible lookups belong
/// behind caching inside the implementation, not in the signature.
pub trait MemorySource: Send + Sync {
    fn retrieve(&self, query: &MemoryQuery<'_>) -> Vec<MemoryItem>;
}

/// Default source: recalls nothing. Keeps the `RetrievedMemory` slot empty
/// until a real backend lands.
pub struct NoopMemorySource;

impl MemorySource for NoopMemorySource {
    fn retrieve(&self, _query: &MemoryQuery<'_>) -> Vec<MemoryItem> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_source_recalls_nothing() {
        let query = MemoryQuery {
            session_id: "test",
            messages: &[],
            reserved_tokens: 1_000,
        };
        assert!(NoopMemorySource.retrieve(&query).is_empty());
    }
}
