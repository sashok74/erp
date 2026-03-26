//! Контракты десериализации для команд и запросов.
//!
//! `FromBody` и `FromQueryParams` — serde-only trait'ы без привязки к HTTP.
//! Используются в delivery-слое (gateway) для типобезопасной десериализации.

use kernel::Command;
use serde::de::DeserializeOwned;

/// Trait для команд, создаваемых из JSON body.
pub trait FromBody: Command {
    /// JSON-тело запроса.
    type Body: DeserializeOwned + Send;
    /// Создать команду из десериализованного body.
    fn from_body(body: Self::Body) -> Self;
}

/// Trait для запросов, создаваемых из query parameters.
pub trait FromQueryParams: Send + Sync {
    /// Тип query-параметров.
    type Params: DeserializeOwned + Send;
    /// Создать запрос из десериализованных параметров.
    fn from_params(params: Self::Params) -> Self;
}
