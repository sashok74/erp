//! Типобезопасные запросы к `common.tenants`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/common/tenants.sql`.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Строка из `common.tenants`.
#[derive(Debug, Clone)]
pub struct TenantRow {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Получить tenant по id.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn get_tenant(
    client: &impl tokio_postgres::GenericClient,
    id: Uuid,
) -> Result<Option<TenantRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, name, slug, is_active, created_at, updated_at \
             FROM common.tenants WHERE id = $1",
            &[&id],
        )
        .await?;

    Ok(row.map(|r| TenantRow {
        id: r.get(0),
        name: r.get(1),
        slug: r.get(2),
        is_active: r.get(3),
        created_at: r.get(4),
        updated_at: r.get(5),
    }))
}

/// Создать tenant. Возвращает созданную строку.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn create_tenant(
    client: &impl tokio_postgres::GenericClient,
    id: Uuid,
    name: &str,
    slug: &str,
) -> Result<TenantRow, tokio_postgres::Error> {
    let r = client
        .query_one(
            "INSERT INTO common.tenants (id, name, slug) \
             VALUES ($1, $2, $3) \
             RETURNING id, name, slug, is_active, created_at, updated_at",
            &[&id, &name, &slug],
        )
        .await?;

    Ok(TenantRow {
        id: r.get(0),
        name: r.get(1),
        slug: r.get(2),
        is_active: r.get(3),
        created_at: r.get(4),
        updated_at: r.get(5),
    })
}
