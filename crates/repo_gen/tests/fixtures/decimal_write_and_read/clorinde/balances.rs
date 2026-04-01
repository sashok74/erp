// Minimal clorinde-gen fixture for decimal_write_and_read test.

#[derive(Debug, Clone, PartialEq)]
pub struct GetBalance {
    pub item_id: uuid::Uuid,
    pub balance: String,
}

pub struct GetBalanceStmt(&'static str, Option<()>);
pub fn get_balance() -> GetBalanceStmt { GetBalanceStmt("", None) }
impl GetBalanceStmt {
    pub fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        item_id: &'a uuid::Uuid,
    ) -> () {
        unimplemented!()
    }
}

pub struct UpsertBalanceStmt(&'static str, Option<()>);
pub fn upsert_balance() -> UpsertBalanceStmt { UpsertBalanceStmt("", None) }
impl UpsertBalanceStmt {
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        item_id: &'a uuid::Uuid,
        balance: &'a T1,
    ) -> Result<u64, tokio_postgres::Error> {
        unimplemented!()
    }
}
