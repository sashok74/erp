#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Audit — structured audit log + domain history writers.
//!
//! `PgAuditLog` реализует `runtime::ports::AuditLog` (запись после commit, best-effort).
//! `DomainHistoryWriter` записывает old/new state снимки (внутри TX).

pub mod history;
pub mod logger;

pub use history::DomainHistoryWriter;
pub use logger::PgAuditLog;
