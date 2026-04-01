//! Two-phase validation: syntax (metadata only) and semantic (with clorinde data).

use anyhow::{Result, bail};

use crate::clorinde_parser::ClorindeQueryInfo;
use crate::sql_parser::{QueryBlock, QueryKind};

// ── Phase 1: Syntax validation (metadata only, no clorinde) ─────────────

/// Validate metadata rules that can be checked without clorinde introspection.
pub fn validate_syntax(block: &QueryBlock) -> Result<()> {
    let m = &block.metadata;

    // Required keys
    if m.repo.is_none() || m.repo.as_ref().is_some_and(|s| s.is_empty()) {
        bail!("missing required metadata key 'repo'");
    }

    let kind = match m.kind {
        Some(k) => k,
        None => bail!("missing required metadata key 'kind'"),
    };

    // dto rules
    if kind == QueryKind::Exec {
        if m.dto.is_some() {
            bail!("'dto' is not allowed for 'exec' queries");
        }
    } else if m.dto.is_none() || m.dto.as_ref().is_some_and(|s| s.is_empty()) {
        bail!("'dto' is required for '{kind}' queries");
    }

    // input rules
    if m.input.is_some() && kind != QueryKind::Exec {
        bail!("'input' is only allowed for 'exec' queries, not '{kind}'");
    }

    Ok(())
}

// ── Phase 2: Semantic validation (with clorinde data) ───────────────────

/// Validate query metadata against clorinde-extracted shape.
pub fn validate_semantic(block: &QueryBlock, clorinde: &ClorindeQueryInfo) -> Result<()> {
    let m = &block.metadata;
    let kind = m.kind.unwrap(); // already validated in syntax phase

    // tenant_id must exist
    if !clorinde.has_tenant_id() {
        bail!("tenant_id not found in clorinde bind params");
    }

    // tenant_id must be first
    if !clorinde.tenant_id_is_first() {
        let first = clorinde
            .all_bind_params
            .first()
            .map(|p| p.name.as_str())
            .unwrap_or("<empty>");
        bail!("tenant_id must be the first bind parameter, but found '{first}' first");
    }

    // dec field validation
    for dec_field in &m.dec {
        if kind == QueryKind::Exec {
            // For exec, dec refers to bind params
            if clorinde.find_bind_param(dec_field).is_none() {
                let param_names: Vec<&str> = clorinde
                    .all_bind_params
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect();
                bail!("dec field '{dec_field}' not found in bind params: {param_names:?}");
            }
        } else {
            // For read queries, dec refers to row fields
            if clorinde.find_row_field(dec_field).is_none() {
                let field_names: Vec<&str> = clorinde
                    .row_fields
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect();
                bail!("dec field '{dec_field}' not found in row fields: {field_names:?}");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clorinde_parser::{ClorindeField, ClorindeParam};
    use crate::sql_parser::Metadata;

    fn make_block(name: &str, kind: QueryKind, dto: Option<&str>) -> QueryBlock {
        QueryBlock {
            name: name.to_string(),
            metadata: Metadata {
                repo: Some("test".to_string()),
                kind: Some(kind),
                dto: dto.map(|s| s.to_string()),
                input: None,
                dec: Vec::new(),
            },
            sql_body: String::new(),
            source_file: "test.sql".to_string(),
            file_stem: "test".to_string(),
        }
    }

    fn make_clorinde(params: &[&str], row_fields: &[&str]) -> ClorindeQueryInfo {
        let all_bind_params: Vec<ClorindeParam> = params
            .iter()
            .map(|&name| ClorindeParam {
                name: name.to_string(),
                rust_type: "uuid::Uuid".to_string(),
                is_string_sql: false,
            })
            .collect();
        let bind_params_without_tenant = all_bind_params
            .iter()
            .filter(|p| p.name != "tenant_id")
            .cloned()
            .collect();
        let row_fields = row_fields
            .iter()
            .map(|&name| ClorindeField {
                name: name.to_string(),
                rust_type: "String".to_string(),
            })
            .collect();
        ClorindeQueryInfo {
            all_bind_params,
            bind_params_without_tenant,
            row_fields,
        }
    }

    // === Syntax tests ===

    #[test]
    fn syntax_valid_exec() {
        let block = make_block("create", QueryKind::Exec, None);
        assert!(validate_syntax(&block).is_ok());
    }

    #[test]
    fn syntax_valid_opt_with_dto() {
        let block = make_block("find", QueryKind::Opt, Some("TestRow"));
        assert!(validate_syntax(&block).is_ok());
    }

    #[test]
    fn syntax_exec_with_dto_fails() {
        let block = make_block("create", QueryKind::Exec, Some("Bad"));
        assert!(validate_syntax(&block).is_err());
    }

    #[test]
    fn syntax_opt_without_dto_fails() {
        let block = make_block("find", QueryKind::Opt, None);
        assert!(validate_syntax(&block).is_err());
    }

    #[test]
    fn syntax_missing_repo_fails() {
        let mut block = make_block("q", QueryKind::Exec, None);
        block.metadata.repo = None;
        assert!(validate_syntax(&block).is_err());
    }

    #[test]
    fn syntax_missing_kind_fails() {
        let mut block = make_block("q", QueryKind::Exec, None);
        block.metadata.kind = None;
        assert!(validate_syntax(&block).is_err());
    }

    #[test]
    fn syntax_input_on_non_exec_fails() {
        let mut block = make_block("q", QueryKind::Opt, Some("Row"));
        block.metadata.input = Some("Bad".to_string());
        assert!(validate_syntax(&block).is_err());
    }

    // === Semantic tests ===

    #[test]
    fn semantic_tenant_id_missing_fails() {
        let block = make_block("q", QueryKind::Exec, None);
        let clorinde = make_clorinde(&["sku"], &[]);
        let err = validate_semantic(&block, &clorinde).unwrap_err();
        assert!(err.to_string().contains("tenant_id"));
    }

    #[test]
    fn semantic_tenant_id_not_first_fails() {
        let block = make_block("q", QueryKind::Exec, None);
        let clorinde = make_clorinde(&["sku", "tenant_id"], &[]);
        let err = validate_semantic(&block, &clorinde).unwrap_err();
        assert!(err.to_string().contains("first"));
    }

    #[test]
    fn semantic_valid_tenant_id_first() {
        let block = make_block("q", QueryKind::Exec, None);
        let clorinde = make_clorinde(&["tenant_id", "sku"], &[]);
        assert!(validate_semantic(&block, &clorinde).is_ok());
    }

    #[test]
    fn semantic_dec_invalid_exec_param_fails() {
        let mut block = make_block("q", QueryKind::Exec, None);
        block.metadata.dec = vec!["missing".to_string()];
        let clorinde = make_clorinde(&["tenant_id", "sku"], &[]);
        let err = validate_semantic(&block, &clorinde).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn semantic_dec_invalid_read_field_fails() {
        let mut block = make_block("q", QueryKind::Opt, Some("Row"));
        block.metadata.dec = vec!["missing".to_string()];
        let clorinde = make_clorinde(&["tenant_id", "sku"], &["id", "sku"]);
        let err = validate_semantic(&block, &clorinde).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn semantic_dec_valid_exec_param() {
        let mut block = make_block("q", QueryKind::Exec, None);
        block.metadata.dec = vec!["balance".to_string()];
        let clorinde = make_clorinde(&["tenant_id", "balance"], &[]);
        assert!(validate_semantic(&block, &clorinde).is_ok());
    }

    #[test]
    fn semantic_dec_valid_read_field() {
        let mut block = make_block("q", QueryKind::Opt, Some("Row"));
        block.metadata.dec = vec!["balance".to_string()];
        let clorinde = make_clorinde(&["tenant_id", "sku"], &["id", "balance"]);
        assert!(validate_semantic(&block, &clorinde).is_ok());
    }
}
