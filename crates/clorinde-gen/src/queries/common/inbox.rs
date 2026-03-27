// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct TryInsertInboxParams<T1: crate::StringSql, T2: crate::StringSql, T3: crate::StringSql> {
    pub event_id: uuid::Uuid,
    pub event_type: T1,
    pub source: T2,
    pub handler_name: T3,
}
#[derive(Debug)]
pub struct CheckProcessedParams<T1: crate::StringSql> {
    pub event_id: uuid::Uuid,
    pub handler_name: T1,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct I32Query<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<i32, tokio_postgres::Error>,
    mapper: fn(i32) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> I32Query<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(i32) -> R) -> I32Query<'c, 'a, 's, C, R, N> {
        I32Query {
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
pub struct TryInsertInboxStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn try_insert_inbox() -> TryInsertInboxStmt {
    TryInsertInboxStmt(
        "INSERT INTO common.inbox (event_id, event_type, source, handler_name) VALUES ($1, $2, $3, $4) ON CONFLICT (event_id, handler_name) DO NOTHING",
        None,
    )
}
impl TryInsertInboxStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        event_id: &'a uuid::Uuid,
        event_type: &'a T1,
        source: &'a T2,
        handler_name: &'a T3,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[event_id, event_type, source, handler_name])
            .await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        TryInsertInboxParams<T1, T2, T3>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for TryInsertInboxStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a TryInsertInboxParams<T1, T2, T3>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.event_id,
            &params.event_type,
            &params.source,
            &params.handler_name,
        ))
    }
}
pub struct CheckProcessedStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn check_processed() -> CheckProcessedStmt {
    CheckProcessedStmt(
        "SELECT 1 FROM common.inbox WHERE event_id = $1 AND handler_name = $2",
        None,
    )
}
impl CheckProcessedStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        event_id: &'a uuid::Uuid,
        handler_name: &'a T1,
    ) -> I32Query<'c, 'a, 's, C, i32, 2> {
        I32Query {
            client,
            params: [event_id, handler_name],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row| Ok(row.try_get(0)?),
            mapper: |it| it,
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        CheckProcessedParams<T1>,
        I32Query<'c, 'a, 's, C, i32, 2>,
        C,
    > for CheckProcessedStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a CheckProcessedParams<T1>,
    ) -> I32Query<'c, 'a, 's, C, i32, 2> {
        self.bind(client, &params.event_id, &params.handler_name)
    }
}
