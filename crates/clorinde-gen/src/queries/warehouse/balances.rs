// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct UpsertBalanceParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub sku: T1,
    pub balance: T2,
    pub last_movement_id: uuid::Uuid,
}
#[derive(Debug)]
pub struct GetBalanceParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub sku: T1,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GetBalance {
    pub item_id: uuid::Uuid,
    pub sku: String,
    pub balance: String,
}
pub struct GetBalanceBorrowed<'a> {
    pub item_id: uuid::Uuid,
    pub sku: &'a str,
    pub balance: &'a str,
}
impl<'a> From<GetBalanceBorrowed<'a>> for GetBalance {
    fn from(
        GetBalanceBorrowed {
            item_id,
            sku,
            balance,
        }: GetBalanceBorrowed<'a>,
    ) -> Self {
        Self {
            item_id,
            sku: sku.into(),
            balance: balance.into(),
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct GetBalanceQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<GetBalanceBorrowed, tokio_postgres::Error>,
    mapper: fn(GetBalanceBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> GetBalanceQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(
        self,
        mapper: fn(GetBalanceBorrowed) -> R,
    ) -> GetBalanceQuery<'c, 'a, 's, C, R, N> {
        GetBalanceQuery {
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
pub struct UpsertBalanceStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn upsert_balance() -> UpsertBalanceStmt {
    UpsertBalanceStmt(
        "INSERT INTO warehouse.inventory_balances (tenant_id, item_id, sku, balance, last_movement_id, updated_at) VALUES ($1, $2, $3, $4::TEXT::NUMERIC, $5, now()) ON CONFLICT (tenant_id, item_id) DO UPDATE SET balance = EXCLUDED.balance, last_movement_id = EXCLUDED.last_movement_id, updated_at = now()",
        None,
    )
}
impl UpsertBalanceStmt {
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
        item_id: &'a uuid::Uuid,
        sku: &'a T1,
        balance: &'a T2,
        last_movement_id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(
                self.0,
                &[tenant_id, item_id, sku, balance, last_movement_id],
            )
            .await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        UpsertBalanceParams<T1, T2>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for UpsertBalanceStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a UpsertBalanceParams<T1, T2>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.tenant_id,
            &params.item_id,
            &params.sku,
            &params.balance,
            &params.last_movement_id,
        ))
    }
}
pub struct GetBalanceStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn get_balance() -> GetBalanceStmt {
    GetBalanceStmt(
        "SELECT item_id, sku, balance::TEXT FROM warehouse.inventory_balances WHERE tenant_id = $1 AND sku = $2",
        None,
    )
}
impl GetBalanceStmt {
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
        sku: &'a T1,
    ) -> GetBalanceQuery<'c, 'a, 's, C, GetBalance, 2> {
        GetBalanceQuery {
            client,
            params: [tenant_id, sku],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<GetBalanceBorrowed, tokio_postgres::Error> {
                    Ok(GetBalanceBorrowed {
                        item_id: row.try_get(0)?,
                        sku: row.try_get(1)?,
                        balance: row.try_get(2)?,
                    })
                },
            mapper: |it| GetBalance::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        GetBalanceParams<T1>,
        GetBalanceQuery<'c, 'a, 's, C, GetBalance, 2>,
        C,
    > for GetBalanceStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a GetBalanceParams<T1>,
    ) -> GetBalanceQuery<'c, 'a, 's, C, GetBalance, 2> {
        self.bind(client, &params.tenant_id, &params.sku)
    }
}
