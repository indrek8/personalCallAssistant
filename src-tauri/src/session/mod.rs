//! Session subsystem.
//!
//! In M1 this is a thin re-export of the domain model. The real
//! `SessionManager` (state machine, thread orchestration, persistence
//! coordination) lands in M2+ per technical-design.md §3.

pub mod manager;
pub mod model;

// Re-exported for ergonomic access from later milestones (SessionManager).
#[allow(unused_imports)]
pub use model::{
    CreatedSession, LabelRef, SessionDraft, SessionFull, SessionMeta, SessionStatus,
    TranscriptEntry,
};
