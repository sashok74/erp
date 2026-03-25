// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct InsertOutboxEntryParams<T1: crate::StringSql, T2: crate::StringSql, T3: crate::JsonSql> {
    pub tenant_id: uuid::Uuid,
    pub event_id: uuid::Uuid,
    pub event_type: T1,
    pub source: T2,
    pub payload: T3,
    pub correlation_id: uuid::Uuid,
    pub causation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GetUnpublishedEvents {
    pub id: i64,
    pub tenant_id: uuid::Uuid,
    pub event_id: uuid::Uuid,
    pub event_type: String,
    pub source: String,
    pub payload: serde_json::Value,
    pub correlation_id: uuid::Uuid,
    pub causation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub retry_count: i32,
}
pub struct GetUnpublishedEventsBorrowed<'a> {
    pub id: i64,
    pub tenant_id: uuid::Uuid,
    pub event_id: uuid::Uuid,
    pub event_type: &'a str,
    pub source: &'a str,
    pub payload: postgres_types::Json<&'a serde_json::value::RawValue>,
    pub correlation_id: uuid::Uuid,
    pub causation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub retry_count: i32,
}
impl<'a> From<GetUnpublishedEventsBorrowed<'a>> for GetUnpublishedEvents {
    fn from(
        GetUnpublishedEventsBorrowed {
            id,
            tenant_id,
            event_id,
            event_type,
            source,
            payload,
            correlation_id,
            causation_id,
            user_id,
            created_at,
            retry_count,
        }: GetUnpublishedEventsBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            tenant_id,
            event_id,
            event_type: event_type.into(),
            source: source.into(),
            payload: serde_json::from_str(payload.0.get()).unwrap(),
            correlation_id,
            causation_id,
            user_id,
            created_at,
            retry_count,
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct I64Query<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<i64, tokio_postgres::Error>,
    mapper: fn(i64) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> I64Query<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(i64) -> R) -> I64Query<'c, 'a, 's, C, R, N> {
        I64Query {
            client: self.client,
            params: self.params,
            query: self.query,
            cached: self.cached,
            extractor: self.extractor,
            mapper,
        }
    }
    pub async fn one(self) -> Result<T, tokio_postgres::Error> {
        let row =
            crate::client::async_::one(self.client, self.query, &self.params, self.cached).await?;
        Ok((self.mapper)((self.extractor)(&row)?))
    }
    pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error> {
        self.iter().await?.try_collect().await
    }
    pub async fn opt(self) -> Result<Option<T>, tokio_postgres::Error> {
        let opt_row =
            crate::client::async_::opt(self.client, self.query, &self.params, self.cached).await?;
        Ok(opt_row
            .map(|row| {
                let extracted = (self.extractor)(&row)?;
                Ok((self.mapper)(extracted))
            })
            .transpose()?)
    }
    pub async fn iter(
        self,
    ) -> Result<
        impl futures::Stream<Item = Result<T, tokio_postgres::Error>> + 'c,
        tokio_postgres::Error,
    > {
        let stream = crate::client::async_::raw(
            self.client,
            self.query,
            crate::slice_iter(&self.params),
            self.cached,
        )
        .await?;
        let mapped = stream
            .map(move |res| {
                res.and_then(|row| {
                    let extracted = (self.extractor)(&row)?;
                    Ok((self.mapper)(extracted))
                })
            })
            .into_stream();
        Ok(mapped)
    }
}
pub struct GetUnpublishedEventsQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor:
        fn(&tokio_postgres::Row) -> Result<GetUnpublishedEventsBorrowed, tokio_postgres::Error>,
    mapper: fn(GetUnpublishedEventsBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> GetUnpublishedEventsQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(
        self,
        mapper: fn(GetUnpublishedEventsBorrowed) -> R,
    ) -> GetUnpublishedEventsQuery<'c, 'a, 's, C, R, N> {
        GetUnpublishedEventsQuery {
            client: self.client,
            params: self.params,
            query: self.query,
            cached: self.cached,
            extractor: self.extractor,
            mapper,
        }
    }
    pub async fn one(self) -> Result<T, tokio_postgres::Error> {
        let row =
            crate::client::async_::one(self.client, self.query, &self.params, self.cached).await?;
        Ok((self.mapper)((self.extractor)(&row)?))
    }
    pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error> {
        self.iter().await?.try_collect().await
    }
    pub async fn opt(self) -> Result<Option<T>, tokio_postgres::Error> {
        let opt_row =
            crate::client::async_::opt(self.client, self.query, &self.params, self.cached).await?;
        Ok(opt_row
            .map(|row| {
                let extracted = (self.extractor)(&row)?;
                Ok((self.mapper)(extracted))
            })
            .transpose()?)
    }
    pub async fn iter(
        self,
    ) -> Result<
        impl futures::Stream<Item = Result<T, tokio_postgres::Error>> + 'c,
        tokio_postgres::Error,
    > {
        let stream = crate::client::async_::raw(
            self.client,
            self.query,
            crate::slice_iter(&self.params),
            self.cached,
        )
        .await?;
        let mapped = stream
            .map(move |res| {
                res.and_then(|row| {
                    let extracted = (self.extractor)(&row)?;
                    Ok((self.mapper)(extracted))
                })
            })
            .into_stream();
        Ok(mapped)
    }
}
pub struct InsertOutboxEntryStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn insert_outbox_entry() -> InsertOutboxEntryStmt {
    InsertOutboxEntryStmt(
        "INSERT INTO common.outbox (tenant_id, event_id, event_type, source, payload, correlation_id, causation_id, user_id, created_at) VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8, $9) RETURNING id",
        None,
    )
}
impl InsertOutboxEntryStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::JsonSql,
    >(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        event_id: &'a uuid::Uuid,
        event_type: &'a T1,
        source: &'a T2,
        payload: &'a T3,
        correlation_id: &'a uuid::Uuid,
        causation_id: &'a uuid::Uuid,
        user_id: &'a uuid::Uuid,
        created_at: &'a chrono::DateTime<chrono::FixedOffset>,
    ) -> I64Query<'c, 'a, 's, C, i64, 9> {
        I64Query {
            client,
            params: [
                tenant_id,
                event_id,
                event_type,
                source,
                payload,
                correlation_id,
                causation_id,
                user_id,
                created_at,
            ],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row| Ok(row.try_get(0)?),
            mapper: |it| it,
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql, T3: crate::JsonSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        InsertOutboxEntryParams<T1, T2, T3>,
        I64Query<'c, 'a, 's, C, i64, 9>,
        C,
    > for InsertOutboxEntryStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a InsertOutboxEntryParams<T1, T2, T3>,
    ) -> I64Query<'c, 'a, 's, C, i64, 9> {
        self.bind(
            client,
            &params.tenant_id,
            &params.event_id,
            &params.event_type,
            &params.source,
            &params.payload,
            &params.correlation_id,
            &params.causation_id,
            &params.user_id,
            &params.created_at,
        )
    }
}
pub struct GetUnpublishedEventsStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn get_unpublished_events() -> GetUnpublishedEventsStmt {
    GetUnpublishedEventsStmt(
        "SELECT id, tenant_id, event_id, event_type, source, payload, correlation_id, causation_id, user_id, created_at, retry_count FROM common.outbox WHERE published = false ORDER BY id LIMIT $1 FOR UPDATE SKIP LOCKED",
        None,
    )
}
impl GetUnpublishedEventsStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        batch_size: &'a i64,
    ) -> GetUnpublishedEventsQuery<'c, 'a, 's, C, GetUnpublishedEvents, 1> {
        GetUnpublishedEventsQuery {
            client,
            params: [batch_size],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |
                row: &tokio_postgres::Row,
            | -> Result<GetUnpublishedEventsBorrowed, tokio_postgres::Error> {
                Ok(GetUnpublishedEventsBorrowed {
                    id: row.try_get(0)?,
                    tenant_id: row.try_get(1)?,
                    event_id: row.try_get(2)?,
                    event_type: row.try_get(3)?,
                    source: row.try_get(4)?,
                    payload: row.try_get(5)?,
                    correlation_id: row.try_get(6)?,
                    causation_id: row.try_get(7)?,
                    user_id: row.try_get(8)?,
                    created_at: row.try_get(9)?,
                    retry_count: row.try_get(10)?,
                })
            },
            mapper: |it| GetUnpublishedEvents::from(it),
        }
    }
}
pub struct MarkPublishedStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn mark_published() -> MarkPublishedStmt {
    MarkPublishedStmt(
        "UPDATE common.outbox SET published = true, published_at = NOW() WHERE id = $1",
        None,
    )
}
impl MarkPublishedStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        id: &'a i64,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[id]).await
    }
}
pub struct IncrementRetryStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn increment_retry() -> IncrementRetryStmt {
    IncrementRetryStmt(
        "UPDATE common.outbox SET retry_count = retry_count + 1 WHERE id = $1",
        None,
    )
}
impl IncrementRetryStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        id: &'a i64,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[id]).await
    }
}
