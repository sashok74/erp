//! Конфигурация приложения из переменных окружения.

/// Настройки ERP Gateway.
pub struct AppConfig {
    /// `PostgreSQL` connection URL.
    pub database_url: String,
    /// JWT signing secret (минимум 32 байта).
    pub jwt_secret: String,
    /// Адрес для HTTP-сервера.
    pub listen_addr: String,
    /// Интервал опроса outbox relay (мс).
    pub relay_poll_ms: u64,
    /// Размер batch для outbox relay.
    pub relay_batch_size: i64,
}

impl AppConfig {
    /// Загрузить конфигурацию из переменных окружения.
    ///
    /// # Panics
    ///
    /// Паникует если обязательные переменные отсутствуют или невалидны.
    #[must_use]
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set (minimum 32 bytes)");

        assert!(
            jwt_secret.len() >= 32,
            "JWT_SECRET must be at least 32 bytes, got {}",
            jwt_secret.len()
        );

        let listen_addr = std::env::var("LISTEN_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        let relay_poll_ms = std::env::var("RELAY_POLL_MS")
            .unwrap_or_else(|_| "500".to_string())
            .parse::<u64>()
            .expect("RELAY_POLL_MS must be a valid u64");

        let relay_batch_size = std::env::var("RELAY_BATCH_SIZE")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<i64>()
            .expect("RELAY_BATCH_SIZE must be a valid i64");

        Self {
            database_url,
            jwt_secret,
            listen_addr,
            relay_poll_ms,
            relay_batch_size,
        }
    }
}
