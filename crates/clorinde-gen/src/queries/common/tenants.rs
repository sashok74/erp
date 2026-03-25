// This file was generated with `clorinde`. Do not modify.

#[derive(Debug)]
pub struct CreateTenantParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub id: uuid::Uuid,
    pub name: T1,
    pub slug: T2,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GetTenant {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}
pub struct GetTenantBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub slug: &'a str,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}
impl<'a> From<GetTenantBorrowed<'a>> for GetTenant {
    fn from(
        GetTenantBorrowed {
            id,
            name,
            slug,
            is_active,
            created_at,
            updated_at,
        }: GetTenantBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            slug: slug.into(),
            is_active,
            created_at,
            updated_at,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTenant {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}
pub struct CreateTenantBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub slug: &'a str,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}
impl<'a> From<CreateTenantBorrowed<'a>> for CreateTenant {
    fn from(
        CreateTenantBorrowed {
            id,
            name,
            slug,
            is_active,
            created_at,
            updated_at,
        }: CreateTenantBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            slug: slug.into(),
            is_active,
            created_at,
            updated_at,
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct GetTenantQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<GetTenantBorrowed, tokio_postgres::Error>,
    mapper: fn(GetTenantBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> GetTenantQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(GetTenantBorrowed) -> R) -> GetTenantQuery<'c, 'a, 's, C, R, N> {
        GetTenantQuery {
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
pub struct CreateTenantQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<CreateTenantBorrowed, tokio_postgres::Error>,
    mapper: fn(CreateTenantBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> CreateTenantQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(
        self,
        mapper: fn(CreateTenantBorrowed) -> R,
    ) -> CreateTenantQuery<'c, 'a, 's, C, R, N> {
        CreateTenantQuery {
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
pub struct GetTenantStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn get_tenant() -> GetTenantStmt {
    GetTenantStmt(
        "SELECT id, name, slug, is_active, created_at, updated_at FROM common.tenants WHERE id = $1",
        None,
    )
}
impl GetTenantStmt {
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
        id: &'a uuid::Uuid,
    ) -> GetTenantQuery<'c, 'a, 's, C, GetTenant, 1> {
        GetTenantQuery {
            client,
            params: [id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<GetTenantBorrowed, tokio_postgres::Error> {
                    Ok(GetTenantBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        slug: row.try_get(2)?,
                        is_active: row.try_get(3)?,
                        created_at: row.try_get(4)?,
                        updated_at: row.try_get(5)?,
                    })
                },
            mapper: |it| GetTenant::from(it),
        }
    }
}
pub struct CreateTenantStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn create_tenant() -> CreateTenantStmt {
    CreateTenantStmt(
        "INSERT INTO common.tenants (id, name, slug) VALUES ($1, $2, $3) RETURNING id, name, slug, is_active, created_at, updated_at",
        None,
    )
}
impl CreateTenantStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        id: &'a uuid::Uuid,
        name: &'a T1,
        slug: &'a T2,
    ) -> CreateTenantQuery<'c, 'a, 's, C, CreateTenant, 3> {
        CreateTenantQuery {
            client,
            params: [id, name, slug],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<CreateTenantBorrowed, tokio_postgres::Error> {
                    Ok(CreateTenantBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        slug: row.try_get(2)?,
                        is_active: row.try_get(3)?,
                        created_at: row.try_get(4)?,
                        updated_at: row.try_get(5)?,
                    })
                },
            mapper: |it| CreateTenant::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        CreateTenantParams<T1, T2>,
        CreateTenantQuery<'c, 'a, 's, C, CreateTenant, 3>,
        C,
    > for CreateTenantStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a CreateTenantParams<T1, T2>,
    ) -> CreateTenantQuery<'c, 'a, 's, C, CreateTenant, 3> {
        self.bind(client, &params.id, &params.name, &params.slug)
    }
}
