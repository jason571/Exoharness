//! Software-engineering policies. No host shell, HTTP, model or filesystem I/O.
//! Persistence/sandbox adapters must independently enforce containment and CAS.
pub mod edits;
pub mod patch;
pub mod paths;
pub mod search;
