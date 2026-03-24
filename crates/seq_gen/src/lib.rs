#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Sequence Generator — gap-free per-tenant document numbering.
//!
//! `SELECT FOR UPDATE` гарантирует отсутствие пропусков.
//! Вызывается внутри `UoW` TX — handler передаёт `GenericClient`.

pub mod generator;

pub use generator::PgSequenceGenerator;
