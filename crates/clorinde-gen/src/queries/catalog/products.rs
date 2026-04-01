// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct CreateProductParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
> {
    pub tenant_id: uuid::Uuid,
    pub id: uuid::Uuid,
    pub sku: T1,
    pub name: T2,
    pub category: T3,
    pub unit: T4,
}
#[derive(Debug)]
pub struct FindBySkuParams<T1: crate::StringSql> {
    pub tenant_id: uuid::Uuid,
    pub sku: T1,
}
#[derive(Clone, Copy, Debug)]
pub struct FindByIdParams {
    pub tenant_id: uuid::Uuid,
    pub id: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FindBySku {
    pub id: uuid::Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}
pub struct FindBySkuBorrowed<'a> {
    pub id: uuid::Uuid,
    pub sku: &'a str,
    pub name: &'a str,
    pub category: &'a str,
    pub unit: &'a str,
}
impl<'a> From<FindBySkuBorrowed<'a>> for FindBySku {
    fn from(
        FindBySkuBorrowed {
            id,
            sku,
            name,
            category,
            unit,
        }: FindBySkuBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            sku: sku.into(),
            name: name.into(),
            category: category.into(),
            unit: unit.into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct FindById {
    pub id: uuid::Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}
pub struct FindByIdBorrowed<'a> {
    pub id: uuid::Uuid,
    pub sku: &'a str,
    pub name: &'a str,
    pub category: &'a str,
    pub unit: &'a str,
}
impl<'a> From<FindByIdBorrowed<'a>> for FindById {
    fn from(
        FindByIdBorrowed {
            id,
            sku,
            name,
            category,
            unit,
        }: FindByIdBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            sku: sku.into(),
            name: name.into(),
            category: category.into(),
            unit: unit.into(),
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct FindBySkuQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<FindBySkuBorrowed, tokio_postgres::Error>,
    mapper: fn(FindBySkuBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> FindBySkuQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(FindBySkuBorrowed) -> R) -> FindBySkuQuery<'c, 'a, 's, C, R, N> {
        FindBySkuQuery {
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
pub struct FindByIdQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<FindByIdBorrowed, tokio_postgres::Error>,
    mapper: fn(FindByIdBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> FindByIdQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(FindByIdBorrowed) -> R) -> FindByIdQuery<'c, 'a, 's, C, R, N> {
        FindByIdQuery {
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
pub struct CreateProductStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn create_product() -> CreateProductStmt {
    CreateProductStmt(
        "INSERT INTO catalog.products (tenant_id, id, sku, name, category, unit) VALUES ($1, $2, $3, $4, $5, $6)",
        None,
    )
}
impl CreateProductStmt {
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
        sku: &'a T1,
        name: &'a T2,
        category: &'a T3,
        unit: &'a T4,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[tenant_id, id, sku, name, category, unit])
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
        CreateProductParams<T1, T2, T3, T4>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for CreateProductStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a CreateProductParams<T1, T2, T3, T4>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.tenant_id,
            &params.id,
            &params.sku,
            &params.name,
            &params.category,
            &params.unit,
        ))
    }
}
pub struct FindBySkuStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn find_by_sku() -> FindBySkuStmt {
    FindBySkuStmt(
        "SELECT id, sku, name, category, unit FROM catalog.products WHERE tenant_id = $1 AND sku = $2",
        None,
    )
}
impl FindBySkuStmt {
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
    ) -> FindBySkuQuery<'c, 'a, 's, C, FindBySku, 2> {
        FindBySkuQuery {
            client,
            params: [tenant_id, sku],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<FindBySkuBorrowed, tokio_postgres::Error> {
                    Ok(FindBySkuBorrowed {
                        id: row.try_get(0)?,
                        sku: row.try_get(1)?,
                        name: row.try_get(2)?,
                        category: row.try_get(3)?,
                        unit: row.try_get(4)?,
                    })
                },
            mapper: |it| FindBySku::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        FindBySkuParams<T1>,
        FindBySkuQuery<'c, 'a, 's, C, FindBySku, 2>,
        C,
    > for FindBySkuStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a FindBySkuParams<T1>,
    ) -> FindBySkuQuery<'c, 'a, 's, C, FindBySku, 2> {
        self.bind(client, &params.tenant_id, &params.sku)
    }
}
pub struct FindByIdStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn find_by_id() -> FindByIdStmt {
    FindByIdStmt(
        "SELECT id, sku, name, category, unit FROM catalog.products WHERE tenant_id = $1 AND id = $2",
        None,
    )
}
impl FindByIdStmt {
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
        tenant_id: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
    ) -> FindByIdQuery<'c, 'a, 's, C, FindById, 2> {
        FindByIdQuery {
            client,
            params: [tenant_id, id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<FindByIdBorrowed, tokio_postgres::Error> {
                    Ok(FindByIdBorrowed {
                        id: row.try_get(0)?,
                        sku: row.try_get(1)?,
                        name: row.try_get(2)?,
                        category: row.try_get(3)?,
                        unit: row.try_get(4)?,
                    })
                },
            mapper: |it| FindById::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        FindByIdParams,
        FindByIdQuery<'c, 'a, 's, C, FindById, 2>,
        C,
    > for FindByIdStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a FindByIdParams,
    ) -> FindByIdQuery<'c, 'a, 's, C, FindById, 2> {
        self.bind(client, &params.tenant_id, &params.id)
    }
}
