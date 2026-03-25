#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Типобезопасные SQL-запросы для ERP.
//!
//! TODO: заменить на автогенерацию Clorinde CLI.
//! Сейчас — ручные struct'ы и функции, повторяющие то,
//! что Clorinde бы сгенерировал из `queries/*.sql`.

pub mod common;
pub mod warehouse;
