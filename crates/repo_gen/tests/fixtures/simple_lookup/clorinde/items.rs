// Minimal clorinde-gen fixture for simple_lookup test.
// Mimics what clorinde would generate for the queries in queries/items.sql.

#[derive(Debug, Clone, PartialEq)]
pub struct FindBySku {
    pub id: uuid::Uuid,
    pub sku: String,
    pub name: String,
}

pub struct FindBySkuStmt(&'static str, Option<()>);
pub fn find_by_sku() -> FindBySkuStmt { FindBySkuStmt("", None) }
impl FindBySkuStmt {
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        sku: &'a T1,
    ) -> () {
        unimplemented!()
    }
}

pub struct CreateItemStmt(&'static str, Option<()>);
pub fn create_item() -> CreateItemStmt { CreateItemStmt("", None) }
impl CreateItemStmt {
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
        sku: &'a T1,
        name: &'a T2,
    ) -> Result<u64, tokio_postgres::Error> {
        unimplemented!()
    }
}
