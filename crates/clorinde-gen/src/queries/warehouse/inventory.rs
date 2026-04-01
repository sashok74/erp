// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct FindItemBySkuParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub sku: T1,
}
#[derive(Debug)]
pub struct CreateItemParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub id: uuid::Uuid,
    pub sku: T1,
}
#[derive(Debug)]
pub struct InsertMovementParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
> {
    pub tenant_id: uuid::Uuid,
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub event_type: T1,
    pub quantity: T2,
    pub balance_after: T3,
    pub doc_number: T4,
    pub correlation_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FindItemBySku {
    pub id: uuid::Uuid,
    pub balance: String,
}
pub struct FindItemBySkuBorrowed<'a> {
    pub id: uuid::Uuid,
    pub balance: &'a str,
}
impl<'a> From<FindItemBySkuBorrowed<'a>> for FindItemBySku {
    fn from(FindItemBySkuBorrowed { id, balance }: FindItemBySkuBorrowed<'a>) -> Self {
        Self {
            id,
            balance: balance.into(),
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct FindItemBySkuQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<FindItemBySkuBorrowed, tokio_postgres::Error>,
    mapper: fn(FindItemBySkuBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> FindItemBySkuQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(
        self,
        mapper: fn(FindItemBySkuBorrowed) -> R,
    ) -> FindItemBySkuQuery<'c, 'a, 's, C, R, N> {
        FindItemBySkuQuery {
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
pub struct FindItemBySkuStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn find_item_by_sku() -> FindItemBySkuStmt {
    FindItemBySkuStmt(
        "SELECT i.id, COALESCE(b.balance, 0)::TEXT AS balance FROM warehouse.inventory_items i LEFT JOIN warehouse.inventory_balances b ON b.tenant_id = i.tenant_id AND b.item_id = i.id WHERE i.tenant_id = $1 AND i.sku = $2",
        None,
    )
}
impl FindItemBySkuStmt {
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
    ) -> FindItemBySkuQuery<'c, 'a, 's, C, FindItemBySku, 2> {
        FindItemBySkuQuery {
            client,
            params: [tenant_id, sku],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<FindItemBySkuBorrowed, tokio_postgres::Error> {
                    Ok(FindItemBySkuBorrowed {
                        id: row.try_get(0)?,
                        balance: row.try_get(1)?,
                    })
                },
            mapper: |it| FindItemBySku::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        FindItemBySkuParams<T1>,
        FindItemBySkuQuery<'c, 'a, 's, C, FindItemBySku, 2>,
        C,
    > for FindItemBySkuStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a FindItemBySkuParams<T1>,
    ) -> FindItemBySkuQuery<'c, 'a, 's, C, FindItemBySku, 2> {
        self.bind(client, &params.tenant_id, &params.sku)
    }
}
pub struct CreateItemStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn create_item() -> CreateItemStmt {
    CreateItemStmt(
        "INSERT INTO warehouse.inventory_items (tenant_id, id, sku) VALUES ($1, $2, $3)",
        None,
    )
}
impl CreateItemStmt {
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
        id: &'a uuid::Uuid,
        sku: &'a T1,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[tenant_id, id, sku]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        CreateItemParams<T1>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for CreateItemStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a CreateItemParams<T1>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.tenant_id, &params.id, &params.sku))
    }
}
pub struct InsertMovementStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn insert_movement() -> InsertMovementStmt {
    InsertMovementStmt(
        "INSERT INTO warehouse.stock_movements (tenant_id, id, item_id, event_type, quantity, balance_after, doc_number, correlation_id, user_id) VALUES ($1, $2, $3, $4, $5::TEXT::NUMERIC, $6::TEXT::NUMERIC, $7, $8, $9)",
        None,
    )
}
impl InsertMovementStmt {
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
        T4: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
        item_id: &'a uuid::Uuid,
        event_type: &'a T1,
        quantity: &'a T2,
        balance_after: &'a T3,
        doc_number: &'a T4,
        correlation_id: &'a uuid::Uuid,
        user_id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(
                self.0,
                &[
                    tenant_id,
                    id,
                    item_id,
                    event_type,
                    quantity,
                    balance_after,
                    doc_number,
                    correlation_id,
                    user_id,
                ],
            )
            .await
    }
}
impl<
        'a,
        C: GenericClient + Send + Sync,
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::StringSql,
        T4: crate::StringSql,
    >
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        InsertMovementParams<T1, T2, T3, T4>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for InsertMovementStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a InsertMovementParams<T1, T2, T3, T4>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.tenant_id,
            &params.id,
            &params.item_id,
            &params.event_type,
            &params.quantity,
            &params.balance_after,
            &params.doc_number,
            &params.correlation_id,
            &params.user_id,
        ))
    }
}
