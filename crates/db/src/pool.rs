//! `PgPool` — обёртка над `deadpool_postgres::Pool`.
//!
//! Единый connection pool для всего приложения, shared через `Arc`.
//! Конфигурируется из `DATABASE_URL`.

use std::str::FromStr;

/// Обёртка над `deadpool_postgres::Pool`.
///
/// Создаётся один раз при старте приложения, используется всеми handler'ами.
/// Каждый checkout — отдельное `PostgreSQL`-соединение с отдельной сессией.
pub struct PgPool {
    pool: deadpool_postgres::Pool,
}

impl PgPool {
    /// Создать pool из `DATABASE_URL`.
    ///
    /// Парсит URL, создаёт pool с `max_size` = 20, connection timeout = 5s.
    ///
    /// # Errors
    ///
    /// Ошибка если URL невалидный или pool не создался.
    pub fn new(database_url: &str) -> Result<Self, anyhow::Error> {
        let pg_config = tokio_postgres::Config::from_str(database_url)?;

        let mgr_config = deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        };
        let mgr =
            deadpool_postgres::Manager::from_config(pg_config, tokio_postgres::NoTls, mgr_config);

        let pool = deadpool_postgres::Pool::builder(mgr).max_size(20).build()?;

        Ok(Self { pool })
    }

    /// Взять соединение из pool'а.
    ///
    /// # Errors
    ///
    /// Ошибка если pool исчерпан или соединение не устанавливается.
    pub async fn get(&self) -> Result<deadpool_postgres::Object, anyhow::Error> {
        Ok(self.pool.get().await?)
    }

    /// Доступ к нижележащему pool'у.
    #[must_use]
    pub fn inner(&self) -> &deadpool_postgres::Pool {
        &self.pool
    }

    /// Проверка доступности `PostgreSQL`: `SELECT 1`.
    ///
    /// # Errors
    ///
    /// Ошибка если БД недоступна.
    pub async fn health_check(&self) -> Result<(), anyhow::Error> {
        let client = self.get().await?;
        client.query_one("SELECT 1", &[]).await?;
        Ok(())
    }
}
