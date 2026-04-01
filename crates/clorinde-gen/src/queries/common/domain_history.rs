// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct InsertDomainHistoryParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::JsonSql,
    T4: crate::JsonSql,
> {
    pub tenant_id: uuid::Uuid,
    pub entity_type: T1,
    pub entity_id: uuid::Uuid,
    pub event_type: T2,
    pub old_state: T3,
    pub new_state: T4,
    pub correlation_id: uuid::Uuid,
    pub causation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
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
pub struct InsertDomainHistoryStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn insert_domain_history() -> InsertDomainHistoryStmt {
    InsertDomainHistoryStmt(
        "INSERT INTO common.domain_history (tenant_id, entity_type, entity_id, event_type, old_state, new_state, correlation_id, causation_id, user_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
        None,
    )
}
impl InsertDomainHistoryStmt {
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
        T4: crate::JsonSql,
    >(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        entity_type: &'a T1,
        entity_id: &'a uuid::Uuid,
        event_type: &'a T2,
        old_state: &'a T3,
        new_state: &'a T4,
        correlation_id: &'a uuid::Uuid,
        causation_id: &'a uuid::Uuid,
        user_id: &'a uuid::Uuid,
        created_at: &'a chrono::DateTime<chrono::FixedOffset>,
    ) -> I64Query<'c, 'a, 's, C, i64, 10> {
        I64Query {
            client,
            params: [
                tenant_id,
                entity_type,
                entity_id,
                event_type,
                old_state,
                new_state,
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
impl<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::JsonSql,
        T4: crate::JsonSql,
    >
    crate::client::async_::Params<
        'c,
        'a,
        's,
        InsertDomainHistoryParams<T1, T2, T3, T4>,
        I64Query<'c, 'a, 's, C, i64, 10>,
        C,
    > for InsertDomainHistoryStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a InsertDomainHistoryParams<T1, T2, T3, T4>,
    ) -> I64Query<'c, 'a, 's, C, i64, 10> {
        self.bind(
            client,
            &params.tenant_id,
            &params.entity_type,
            &params.entity_id,
            &params.event_type,
            &params.old_state,
            &params.new_state,
            &params.correlation_id,
            &params.causation_id,
            &params.user_id,
            &params.created_at,
        )
    }
}
