#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

//! Event Bus — межмодульное взаимодействие через события.
//!
//! Trait `EventBus` определяет контракт. `InProcessBus` — реализация
//! для modular monolith на tokio channels.
//!
//! При переходе к микросервисам: заменить `InProcessBus` на `RabbitMqBus`,
//! `NatsBus` или `KafkaBus` — domain и application код не меняется.

pub mod bus;
pub mod envelope;
pub mod registry;
pub mod traits;

pub use bus::InProcessBus;
pub use envelope::EventEnvelope;
pub use registry::{ErasedEventHandler, EventHandlerAdapter, HandlerRegistry};
pub use traits::{EventBus, EventHandler};
