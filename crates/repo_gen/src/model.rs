//! Resolved model: merges SQL metadata + clorinde types into generation-ready model.

use std::collections::HashMap;

use anyhow::Result;

use crate::clorinde_parser::ClorindeQueryInfo;
use crate::sql_parser::{QueryBlock, QueryKind};

#[derive(Debug)]
pub struct ResolvedQuery {
    pub bc: String,
    pub file_stem: String,
    pub name: String,
    pub repo_group: String,
    pub kind: QueryKind,
    pub dto_name: Option<String>,
    pub input_name: Option<String>,
    /// Params excluding tenant_id, with domain types applied.
    pub params: Vec<ResolvedParam>,
    /// Row fields for read queries, with domain types applied.
    pub row_fields: Vec<ResolvedField>,
    /// Full clorinde function path.
    pub clorinde_fn: String,
}

#[derive(Debug)]
pub struct ResolvedParam {
    pub name: String,
    /// Domain-facing Rust type (e.g. `&uuid::Uuid`, `&str`, `&bigdecimal::BigDecimal`).
    pub domain_type: String,
    /// Whether this param needs DecStr wrapping for bind.
    pub needs_dec_str: bool,
    /// Whether this param is a string type in clorinde.
    pub is_string_sql: bool,
}

#[derive(Debug)]
pub struct ResolvedField {
    pub name: String,
    /// Domain-facing Rust type (e.g. `uuid::Uuid`, `String`, `bigdecimal::BigDecimal`).
    pub domain_type: String,
    /// Whether this field needs parse_dec conversion from String.
    pub needs_parse_dec: bool,
}

pub fn resolve(
    bc: &str,
    blocks: &[QueryBlock],
    clorinde: &HashMap<String, ClorindeQueryInfo>,
) -> Result<Vec<ResolvedQuery>> {
    let mut result = Vec::new();

    for block in blocks {
        let info = clorinde.get(&block.name).ok_or_else(|| {
            anyhow::anyhow!(
                "no clorinde info for query '{}' — is clorinde-gen up to date?",
                block.name
            )
        })?;

        let kind = block.metadata.kind.unwrap();
        let dec_set: std::collections::HashSet<&str> =
            block.metadata.dec.iter().map(|s| s.as_str()).collect();

        // Use clorinde data (without tenant_id) for params
        let params = resolve_params(&info.bind_params_without_tenant, &dec_set);

        // Use clorinde row fields for read queries
        let row_fields = if kind.is_read() {
            resolve_fields(&info.row_fields, &dec_set)
        } else {
            Vec::new()
        };

        let clorinde_fn = format!(
            "clorinde_gen::queries::{bc}::{}::{}",
            block.file_stem, block.name
        );

        result.push(ResolvedQuery {
            bc: bc.to_string(),
            file_stem: block.file_stem.clone(),
            name: block.name.clone(),
            repo_group: block.metadata.repo.clone().unwrap(),
            kind,
            dto_name: block.metadata.dto.clone(),
            input_name: block.metadata.input.clone(),
            params,
            row_fields,
            clorinde_fn,
        });
    }

    Ok(result)
}

fn resolve_params(
    clorinde_params: &[crate::clorinde_parser::ClorindeParam],
    dec_set: &std::collections::HashSet<&str>,
) -> Vec<ResolvedParam> {
    clorinde_params
        .iter()
        .map(|cp| {
            let is_dec = dec_set.contains(cp.name.as_str());
            let domain_type = if is_dec {
                "&bigdecimal::BigDecimal".to_string()
            } else if cp.is_string_sql {
                "&str".to_string()
            } else {
                format!("&{}", cp.rust_type)
            };

            ResolvedParam {
                name: cp.name.clone(),
                domain_type,
                needs_dec_str: is_dec,
                is_string_sql: cp.is_string_sql,
            }
        })
        .collect()
}

fn resolve_fields(
    clorinde_fields: &[crate::clorinde_parser::ClorindeField],
    dec_set: &std::collections::HashSet<&str>,
) -> Vec<ResolvedField> {
    clorinde_fields
        .iter()
        .map(|cf| {
            let is_dec = dec_set.contains(cf.name.as_str());
            let domain_type = if is_dec {
                "bigdecimal::BigDecimal".to_string()
            } else {
                cf.rust_type.clone()
            };

            ResolvedField {
                name: cf.name.clone(),
                domain_type,
                needs_parse_dec: is_dec,
            }
        })
        .collect()
}
