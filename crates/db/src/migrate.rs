//! Минимальный SQL migration runner.
//!
//! Читает `.sql` файлы из указанного каталога, сортирует по имени,
//! выполняет те, что ещё не применены. Состояние хранится в `common._migrations`.

use tracing::info;

use crate::pool::PgPool;

/// Применить SQL-миграции из каталога.
///
/// Создаёт служебную таблицу `common._migrations` если нет.
/// Пропускает уже применённые миграции. Идемпотентен.
///
/// # Errors
///
/// Ошибка если SQL-файл невалидный или БД недоступна.
pub async fn run_migrations(pool: &PgPool, migrations_dir: &str) -> Result<(), anyhow::Error> {
    let client = pool.get().await?;

    // Создаём таблицу для трекинга миграций (если ещё нет).
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS common._migrations (
                name       TEXT PRIMARY KEY,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .await?;

    // Читаем .sql файлы из каталога, сортируем по имени.
    let mut entries: Vec<_> = std::fs::read_dir(migrations_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "sql"))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Проверяем, была ли миграция уже применена.
        let already_applied = client
            .query_opt(
                "SELECT 1 FROM common._migrations WHERE name = $1",
                &[&file_name],
            )
            .await?
            .is_some();

        if already_applied {
            info!(migration = %file_name, "skipped (already applied)");
            continue;
        }

        // Читаем и выполняем SQL.
        let sql = std::fs::read_to_string(entry.path())?;
        client.batch_execute(&sql).await?;

        // Записываем в трекинг.
        client
            .execute(
                "INSERT INTO common._migrations (name) VALUES ($1)",
                &[&file_name],
            )
            .await?;

        info!(migration = %file_name, "applied");
    }

    Ok(())
}
