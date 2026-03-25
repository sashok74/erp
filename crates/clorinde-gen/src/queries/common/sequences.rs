// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct NextValueParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub seq_name: T1,
}
#[derive(Debug)]
pub struct IncrementSequenceParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub seq_name: T1,
}
#[derive(Debug)]
pub struct EnsureSequenceParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub seq_name: T1,
    pub prefix: T2,
}
#[derive(Debug, Clone, PartialEq)]
pub struct NextValue {
    pub prefix: String,
    pub next_value: i64,
}
pub struct NextValueBorrowed<'a> {
    pub prefix: &'a str,
    pub next_value: i64,
}
impl<'a> From<NextValueBorrowed<'a>> for NextValue {
    fn from(NextValueBorrowed { prefix, next_value }: NextValueBorrowed<'a>) -> Self {
        Self {
            prefix: prefix.into(),
            next_value,
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct NextValueQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<NextValueBorrowed, tokio_postgres::Error>,
    mapper: fn(NextValueBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> NextValueQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(NextValueBorrowed) -> R) -> NextValueQuery<'c, 'a, 's, C, R, N> {
        NextValueQuery {
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
pub struct NextValueStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn next_value() -> NextValueStmt {
    NextValueStmt(
        "SELECT prefix, next_value FROM common.sequences WHERE tenant_id = $1 AND seq_name = $2 FOR UPDATE",
        None,
    )
}
impl NextValueStmt {
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
        tenant_id: &'a uuid::Uuid,
        seq_name: &'a T1,
    ) -> NextValueQuery<'c, 'a, 's, C, NextValue, 2> {
        NextValueQuery {
            client,
            params: [tenant_id, seq_name],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<NextValueBorrowed, tokio_postgres::Error> {
                    Ok(NextValueBorrowed {
                        prefix: row.try_get(0)?,
                        next_value: row.try_get(1)?,
                    })
                },
            mapper: |it| NextValue::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        NextValueParams<T1>,
        NextValueQuery<'c, 'a, 's, C, NextValue, 2>,
        C,
    > for NextValueStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a NextValueParams<T1>,
    ) -> NextValueQuery<'c, 'a, 's, C, NextValue, 2> {
        self.bind(client, &params.tenant_id, &params.seq_name)
    }
}
pub struct IncrementSequenceStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn increment_sequence() -> IncrementSequenceStmt {
    IncrementSequenceStmt(
        "UPDATE common.sequences SET next_value = next_value + 1 WHERE tenant_id = $1 AND seq_name = $2",
        None,
    )
}
impl IncrementSequenceStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        seq_name: &'a T1,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[tenant_id, seq_name]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        IncrementSequenceParams<T1>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for IncrementSequenceStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a IncrementSequenceParams<T1>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.tenant_id, &params.seq_name))
    }
}
pub struct EnsureSequenceStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn ensure_sequence() -> EnsureSequenceStmt {
    EnsureSequenceStmt(
        "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) VALUES ($1, $2, $3, 1) ON CONFLICT (tenant_id, seq_name) DO NOTHING",
        None,
    )
}
impl EnsureSequenceStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        seq_name: &'a T1,
        prefix: &'a T2,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[tenant_id, seq_name, prefix]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        EnsureSequenceParams<T1, T2>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for EnsureSequenceStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a EnsureSequenceParams<T1, T2>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.tenant_id, &params.seq_name, &params.prefix))
    }
}
