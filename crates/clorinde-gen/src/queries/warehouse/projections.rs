// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct UpsertProductProjectionParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
> {
    pub tenant_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
    pub sku: T1,
    pub name: T2,
    pub category: T3,
}
#[derive(Debug)]
pub struct GetProjectionBySkuParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub sku: T1,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GetProjectionBySku {
    pub product_id: uuid::Uuid,
    pub name: String,
    pub category: String,
}
pub struct GetProjectionBySkuBorrowed<'a> {
    pub product_id: uuid::Uuid,
    pub name: &'a str,
    pub category: &'a str,
}
impl<'a> From<GetProjectionBySkuBorrowed<'a>> for GetProjectionBySku {
    fn from(
        GetProjectionBySkuBorrowed {
            product_id,
            name,
            category,
        }: GetProjectionBySkuBorrowed<'a>,
    ) -> Self {
        Self {
            product_id,
            name: name.into(),
            category: category.into(),
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct GetProjectionBySkuQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor:
        fn(&tokio_postgres::Row) -> Result<GetProjectionBySkuBorrowed, tokio_postgres::Error>,
    mapper: fn(GetProjectionBySkuBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> GetProjectionBySkuQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(
        self,
        mapper: fn(GetProjectionBySkuBorrowed) -> R,
    ) -> GetProjectionBySkuQuery<'c, 'a, 's, C, R, N> {
        GetProjectionBySkuQuery {
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
pub struct UpsertProductProjectionStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn upsert_product_projection() -> UpsertProductProjectionStmt {
    UpsertProductProjectionStmt(
        "INSERT INTO warehouse.product_projections (tenant_id, product_id, sku, name, category, updated_at) VALUES ($1, $2, $3, $4, $5, now()) ON CONFLICT (tenant_id, product_id) DO UPDATE SET sku = EXCLUDED.sku, name = EXCLUDED.name, category = EXCLUDED.category, updated_at = now()",
        None,
    )
}
impl UpsertProductProjectionStmt {
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
        tenant_id: &'a uuid::Uuid,
        product_id: &'a uuid::Uuid,
        sku: &'a T1,
        name: &'a T2,
        category: &'a T3,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[tenant_id, product_id, sku, name, category])
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
        UpsertProductProjectionParams<T1, T2, T3>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for UpsertProductProjectionStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a UpsertProductProjectionParams<T1, T2, T3>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.tenant_id,
            &params.product_id,
            &params.sku,
            &params.name,
            &params.category,
        ))
    }
}
pub struct GetProjectionBySkuStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn get_projection_by_sku() -> GetProjectionBySkuStmt {
    GetProjectionBySkuStmt(
        "SELECT product_id, name, category FROM warehouse.product_projections WHERE tenant_id = $1 AND sku = $2",
        None,
    )
}
impl GetProjectionBySkuStmt {
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
    ) -> GetProjectionBySkuQuery<'c, 'a, 's, C, GetProjectionBySku, 2> {
        GetProjectionBySkuQuery {
            client,
            params: [tenant_id, sku],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |
                row: &tokio_postgres::Row,
            | -> Result<GetProjectionBySkuBorrowed, tokio_postgres::Error> {
                Ok(GetProjectionBySkuBorrowed {
                    product_id: row.try_get(0)?,
                    name: row.try_get(1)?,
                    category: row.try_get(2)?,
                })
            },
            mapper: |it| GetProjectionBySku::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        GetProjectionBySkuParams<T1>,
        GetProjectionBySkuQuery<'c, 'a, 's, C, GetProjectionBySku, 2>,
        C,
    > for GetProjectionBySkuStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a GetProjectionBySkuParams<T1>,
    ) -> GetProjectionBySkuQuery<'c, 'a, 's, C, GetProjectionBySku, 2> {
        self.bind(client, &params.tenant_id, &params.sku)
    }
}
