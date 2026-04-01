//! Parses clorinde-gen Rust files to extract param/row struct info via `syn`.
//!
//! This is the source of truth for query shape: bind params, row fields, types.
//! The SQL parser only provides block boundaries and metadata.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use heck::ToUpperCamelCase;

use crate::sql_parser::QueryBlock;

/// Info extracted from clorinde-gen for a single query.
#[derive(Debug, Clone)]
pub struct ClorindeQueryInfo {
    /// All bind parameters in order, INCLUDING tenant_id.
    pub all_bind_params: Vec<ClorindeParam>,
    /// Bind parameters WITHOUT tenant_id (for generated method signatures).
    pub bind_params_without_tenant: Vec<ClorindeParam>,
    /// Fields of the owned Row struct (for read queries). Empty for exec.
    pub row_fields: Vec<ClorindeField>,
}

impl ClorindeQueryInfo {
    /// Whether tenant_id is present in the bind params.
    pub fn has_tenant_id(&self) -> bool {
        self.all_bind_params.iter().any(|p| p.name == "tenant_id")
    }

    /// Whether tenant_id is the first bind param.
    pub fn tenant_id_is_first(&self) -> bool {
        self.all_bind_params
            .first()
            .is_some_and(|p| p.name == "tenant_id")
    }

    /// Find a bind param by name (searches all params including tenant_id).
    pub fn find_bind_param(&self, name: &str) -> Option<&ClorindeParam> {
        self.all_bind_params.iter().find(|p| p.name == name)
    }

    /// Find a row field by name.
    pub fn find_row_field(&self, name: &str) -> Option<&ClorindeField> {
        self.row_fields.iter().find(|f| f.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct ClorindeParam {
    pub name: String,
    /// The Rust type as string (e.g. `"uuid::Uuid"`, `"T1"`).
    pub rust_type: String,
    /// Whether the param is generic (T: StringSql).
    pub is_string_sql: bool,
}

#[derive(Debug, Clone)]
pub struct ClorindeField {
    pub name: String,
    pub rust_type: String,
}

/// Parse clorinde-gen for all queries in a BC.
/// Returns a map from query_name -> ClorindeQueryInfo.
pub fn parse_bc_clorinde(
    clorinde_dir: &Path,
    blocks: &[QueryBlock],
) -> Result<HashMap<String, ClorindeQueryInfo>> {
    // Group blocks by file_stem to parse each clorinde file once
    let mut by_file: HashMap<String, Vec<&QueryBlock>> = HashMap::new();
    for block in blocks {
        by_file
            .entry(block.file_stem.clone())
            .or_default()
            .push(block);
    }

    let mut result = HashMap::new();

    for (file_stem, file_blocks) in &by_file {
        let rs_path = clorinde_dir.join(format!("{file_stem}.rs"));
        if !rs_path.exists() {
            bail!(
                "clorinde-gen file not found: {}. Run `just clorinde-generate` first.",
                rs_path.display()
            );
        }

        let source = std::fs::read_to_string(&rs_path)
            .with_context(|| format!("reading {}", rs_path.display()))?;
        let syntax =
            syn::parse_file(&source).with_context(|| format!("parsing {}", rs_path.display()))?;

        for block in file_blocks {
            let info = extract_query_info(&syntax, &block.name).with_context(|| {
                format!(
                    "extracting clorinde info for query '{}' from {}",
                    block.name,
                    rs_path.display()
                )
            })?;
            result.insert(block.name.clone(), info);
        }
    }

    Ok(result)
}

/// Parse a clorinde Rust source string (for testing without filesystem).
#[cfg(test)]
pub fn parse_clorinde_source(source: &str, query_name: &str) -> Result<ClorindeQueryInfo> {
    let syntax = syn::parse_file(source).context("parsing clorinde source")?;
    extract_query_info(&syntax, query_name)
}

fn extract_query_info(file: &syn::File, query_name: &str) -> Result<ClorindeQueryInfo> {
    let camel = query_name.to_upper_camel_case();
    let stmt_name = format!("{camel}Stmt");

    // Extract all bind params including tenant_id
    let all_bind_params = extract_bind_params(file, &stmt_name)
        .with_context(|| format!("extracting bind params from {stmt_name}"))?;

    // Build filtered list without tenant_id
    let bind_params_without_tenant = all_bind_params
        .iter()
        .filter(|p| p.name != "tenant_id")
        .cloned()
        .collect();

    // Extract row fields for read queries
    let row_fields = extract_row_fields(file, &camel);

    Ok(ClorindeQueryInfo {
        all_bind_params,
        bind_params_without_tenant,
        row_fields,
    })
}

/// Extract parameters from a Stmt::bind() method, skipping `&self` and `client`.
fn extract_bind_params(file: &syn::File, stmt_name: &str) -> Result<Vec<ClorindeParam>> {
    for item in &file.items {
        let syn::Item::Impl(impl_block) = item else {
            continue;
        };

        let syn::Type::Path(type_path) = impl_block.self_ty.as_ref() else {
            continue;
        };
        let self_ident = type_path.path.segments.last().map(|s| s.ident.to_string());
        if self_ident.as_deref() != Some(stmt_name) {
            continue;
        }

        for item in &impl_block.items {
            let syn::ImplItem::Fn(method) = item else {
                continue;
            };
            if method.sig.ident != "bind" {
                continue;
            }

            return parse_bind_signature(&method.sig);
        }
    }

    bail!("bind method not found in {stmt_name}");
}

fn parse_bind_signature(sig: &syn::Signature) -> Result<Vec<ClorindeParam>> {
    let mut params = Vec::new();

    for input in &sig.inputs {
        let syn::FnArg::Typed(pat_type) = input else {
            // &self
            continue;
        };

        let name = match pat_type.pat.as_ref() {
            syn::Pat::Ident(ident) => ident.ident.to_string(),
            _ => continue,
        };

        // Skip client — it's infrastructure, not a query param
        if name == "client" {
            continue;
        }

        let (rust_type, is_string_sql) = classify_param_type(&pat_type.ty);
        params.push(ClorindeParam {
            name,
            rust_type,
            is_string_sql,
        });
    }

    Ok(params)
}

/// Classify a parameter type from the bind signature.
fn classify_param_type(ty: &syn::Type) -> (String, bool) {
    if let syn::Type::Reference(ref_ty) = ty {
        return classify_param_type(&ref_ty.elem);
    }

    match ty {
        syn::Type::Path(type_path) => {
            let type_str = path_to_string(&type_path.path);
            // Check if it's a generic T (like T1, T2, T3, T4)
            if type_str.len() <= 2
                && type_str.starts_with('T')
                && type_str[1..].chars().all(|c| c.is_ascii_digit())
            {
                return (type_str, true);
            }
            (type_str, false)
        }
        _ => (quote_type(ty), false),
    }
}

/// Extract fields from the owned Row struct (e.g., `pub struct GetBalance { ... }`).
fn extract_row_fields(file: &syn::File, camel_name: &str) -> Vec<ClorindeField> {
    for item in &file.items {
        let syn::Item::Struct(s) = item else {
            continue;
        };
        if s.ident != camel_name {
            continue;
        }
        let syn::Fields::Named(fields) = &s.fields else {
            continue;
        };

        return fields
            .named
            .iter()
            .filter_map(|f| {
                let name = f.ident.as_ref()?.to_string();
                let rust_type = quote_type(&f.ty);
                Some(ClorindeField { name, rust_type })
            })
            .collect();
    }
    Vec::new()
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn quote_type(ty: &syn::Type) -> String {
    use proc_macro2::TokenStream;
    use quote::ToTokens;
    let mut tokens = TokenStream::new();
    ty.to_tokens(&mut tokens);
    tokens.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal clorinde-gen source for an exec query (INSERT).
    const EXEC_SOURCE: &str = r#"
pub struct CreateItemStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn create_item() -> CreateItemStmt { CreateItemStmt("", None) }
impl CreateItemStmt {
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
        sku: &'a T1,
    ) -> Result<u64, tokio_postgres::Error> {
        unimplemented!()
    }
}
"#;

    /// Minimal clorinde-gen source for a read query (SELECT).
    const READ_SOURCE: &str = r#"
pub struct GetBalance {
    pub item_id: uuid::Uuid,
    pub sku: String,
    pub balance: String,
}
pub struct GetBalanceStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn get_balance() -> GetBalanceStmt { GetBalanceStmt("", None) }
impl GetBalanceStmt {
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        tenant_id: &'a uuid::Uuid,
        sku: &'a T1,
    ) -> GetBalanceQuery<'c, 'a, 's, C, GetBalance, 2> {
        unimplemented!()
    }
}
"#;

    #[test]
    fn exec_bind_params_include_tenant_id() {
        let info = parse_clorinde_source(EXEC_SOURCE, "create_item").unwrap();
        assert_eq!(info.all_bind_params.len(), 3); // tenant_id, id, sku
        assert_eq!(info.all_bind_params[0].name, "tenant_id");
        assert_eq!(info.all_bind_params[1].name, "id");
        assert_eq!(info.all_bind_params[2].name, "sku");
    }

    #[test]
    fn exec_bind_params_without_tenant() {
        let info = parse_clorinde_source(EXEC_SOURCE, "create_item").unwrap();
        assert_eq!(info.bind_params_without_tenant.len(), 2); // id, sku
        assert_eq!(info.bind_params_without_tenant[0].name, "id");
    }

    #[test]
    fn tenant_id_is_first() {
        let info = parse_clorinde_source(EXEC_SOURCE, "create_item").unwrap();
        assert!(info.has_tenant_id());
        assert!(info.tenant_id_is_first());
    }

    #[test]
    fn read_query_row_fields() {
        let info = parse_clorinde_source(READ_SOURCE, "get_balance").unwrap();
        assert_eq!(info.row_fields.len(), 3);
        assert_eq!(info.row_fields[0].name, "item_id");
        assert_eq!(info.row_fields[1].name, "sku");
        assert_eq!(info.row_fields[2].name, "balance");
    }

    #[test]
    fn exec_no_row_fields() {
        let info = parse_clorinde_source(EXEC_SOURCE, "create_item").unwrap();
        assert!(info.row_fields.is_empty());
    }

    #[test]
    fn string_sql_param_detected() {
        let info = parse_clorinde_source(EXEC_SOURCE, "create_item").unwrap();
        let sku = &info.all_bind_params[2];
        assert_eq!(sku.name, "sku");
        assert!(sku.is_string_sql);
    }

    #[test]
    fn uuid_param_not_string_sql() {
        let info = parse_clorinde_source(EXEC_SOURCE, "create_item").unwrap();
        let id = &info.all_bind_params[1];
        assert_eq!(id.name, "id");
        assert!(!id.is_string_sql);
    }
}
