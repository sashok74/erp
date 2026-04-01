//! SQL file parser: extracts query blocks with inline metadata.
//!
//! This parser is intentionally simple — it only splits SQL files into
//! query blocks and parses `--@` metadata. It does NOT analyze SQL shape
//! (params, columns). That responsibility belongs to `clorinde_parser`.

use std::path::Path;

use anyhow::{Result, bail};

/// A single query block parsed from a SQL file.
#[derive(Debug, Clone)]
pub struct QueryBlock {
    /// Query name from `--! query_name`.
    pub name: String,
    /// Parsed metadata from `--@` lines.
    pub metadata: Metadata,
    /// Raw SQL body (everything after metadata lines, joined).
    /// Retained for diagnostics and future code-emission use.
    #[allow(dead_code)]
    pub sql_body: String,
    /// Source file name (e.g. `"inventory.sql"`).
    pub source_file: String,
    /// File stem without extension (e.g. `"inventory"`).
    pub file_stem: String,
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub repo: Option<String>,
    pub kind: Option<QueryKind>,
    pub dto: Option<String>,
    pub input: Option<String>,
    pub dec: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    Exec,
    Opt,
    One,
    All,
}

impl QueryKind {
    pub fn is_read(self) -> bool {
        matches!(self, Self::Opt | Self::One | Self::All)
    }
}

impl std::fmt::Display for QueryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exec => write!(f, "exec"),
            Self::Opt => write!(f, "opt"),
            Self::One => write!(f, "one"),
            Self::All => write!(f, "all"),
        }
    }
}

/// Parse all `.sql` files in a BC queries directory.
pub fn parse_bc_sql_files(dir: &Path) -> Result<Vec<QueryBlock>> {
    let mut blocks = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "sql"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let content = std::fs::read_to_string(&path)?;
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let mut file_blocks = parse_sql_file(&content, &file_name, &file_stem)?;
        blocks.append(&mut file_blocks);
    }
    Ok(blocks)
}

/// Parse a single SQL file into query blocks.
pub fn parse_sql_file(content: &str, file_name: &str, file_stem: &str) -> Result<Vec<QueryBlock>> {
    let mut blocks = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_meta_lines: Vec<String> = Vec::new();
    let mut current_sql_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(name) = trimmed.strip_prefix("--!") {
            // Flush previous block
            if let Some(prev_name) = current_name.take() {
                blocks.push(build_block(
                    prev_name,
                    &current_meta_lines,
                    &current_sql_lines,
                    file_name,
                    file_stem,
                )?);
            }
            // Start new block — strip clorinde type annotations like `: (balance)`
            let raw_name = name.trim();
            let query_name = raw_name.split_whitespace().next().unwrap_or(raw_name);
            current_name = Some(query_name.to_string());
            current_meta_lines.clear();
            current_sql_lines.clear();
        } else if trimmed.starts_with("--@") && current_name.is_some() {
            current_meta_lines.push(trimmed.to_string());
        } else if current_name.is_some() {
            // Collect all lines (including empty and comments) as SQL body
            current_sql_lines.push(line.to_string());
        }
    }

    // Flush last block
    if let Some(name) = current_name.take() {
        blocks.push(build_block(
            name,
            &current_meta_lines,
            &current_sql_lines,
            file_name,
            file_stem,
        )?);
    }

    Ok(blocks)
}

fn build_block(
    name: String,
    meta_lines: &[String],
    sql_lines: &[String],
    file_name: &str,
    file_stem: &str,
) -> Result<QueryBlock> {
    let metadata = parse_metadata(meta_lines)?;
    let sql_body = sql_lines.join("\n");

    Ok(QueryBlock {
        name,
        metadata,
        sql_body,
        source_file: file_name.to_string(),
        file_stem: file_stem.to_string(),
    })
}

/// Parse `--@` metadata lines into a Metadata struct.
pub fn parse_metadata(lines: &[String]) -> Result<Metadata> {
    let mut repo = None;
    let mut kind = None;
    let mut dto = None;
    let mut input = None;
    let mut dec = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();

    for line in lines {
        let body = line.strip_prefix("--@").unwrap().trim();
        let (key, value) = body
            .split_once(':')
            .map(|(k, v)| (k.trim(), v.trim()))
            .unwrap_or((body, ""));

        // Check for duplicate keys
        if !seen_keys.insert(key.to_string()) {
            bail!("duplicate metadata key: '{key}'");
        }

        match key {
            "repo" => repo = Some(value.to_string()),
            "kind" => {
                kind = Some(match value {
                    "exec" => QueryKind::Exec,
                    "opt" => QueryKind::Opt,
                    "one" => QueryKind::One,
                    "all" => QueryKind::All,
                    other => bail!("unknown kind: '{other}'. Expected: exec|opt|one|all"),
                });
            }
            "dto" => dto = Some(value.to_string()),
            "input" => input = Some(value.to_string()),
            "dec" => {
                dec = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            unknown => bail!("unknown metadata key: '{unknown}'"),
        }
    }

    Ok(Metadata {
        repo,
        kind,
        dto,
        input,
        dec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Block parser tests ===

    #[test]
    fn single_file_multiple_blocks() {
        let sql = "\
--! create_item
--@ repo: inventory
--@ kind: exec
INSERT INTO t (tenant_id, id) VALUES (:tenant_id, :id);

--! find_by_sku
--@ repo: inventory
--@ kind: opt
--@ dto: ItemRow
SELECT id, sku FROM t WHERE tenant_id = :tenant_id AND sku = :sku;
";
        let blocks = parse_sql_file(sql, "inventory.sql", "inventory").unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "create_item");
        assert_eq!(blocks[1].name, "find_by_sku");
    }

    #[test]
    fn query_name_strips_clorinde_annotations() {
        let sql = "--! get_balance : (balance)\n--@ repo: b\n--@ kind: opt\n--@ dto: R\nSELECT 1;";
        let blocks = parse_sql_file(sql, "t.sql", "t").unwrap();
        assert_eq!(blocks[0].name, "get_balance");
    }

    #[test]
    fn metadata_order_does_not_matter() {
        let sql = "--! q\n--@ kind: exec\n--@ repo: r\nINSERT 1;";
        let blocks = parse_sql_file(sql, "t.sql", "t").unwrap();
        let m = &blocks[0].metadata;
        assert_eq!(m.repo.as_deref(), Some("r"));
        assert_eq!(m.kind, Some(QueryKind::Exec));
    }

    #[test]
    fn empty_lines_between_metadata_and_sql_ok() {
        let sql = "--! q\n--@ repo: r\n--@ kind: exec\n\n\nINSERT 1;";
        let blocks = parse_sql_file(sql, "t.sql", "t").unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].sql_body.contains("INSERT"));
    }

    #[test]
    fn block_comments_in_sql_body_preserved() {
        let sql = "--! q\n--@ repo: r\n--@ kind: exec\n/* block comment */\nINSERT 1;";
        let blocks = parse_sql_file(sql, "t.sql", "t").unwrap();
        assert!(blocks[0].sql_body.contains("/* block comment */"));
    }

    #[test]
    fn sql_body_contains_raw_sql() {
        let sql = "--! q\n--@ repo: r\n--@ kind: exec\nINSERT INTO t (a, b)\nVALUES (:a, :b);";
        let blocks = parse_sql_file(sql, "t.sql", "t").unwrap();
        assert!(blocks[0].sql_body.contains(":a"));
        assert!(blocks[0].sql_body.contains(":b"));
    }

    // === Metadata parser tests ===

    #[test]
    fn unknown_key_error() {
        let lines = vec!["--@ repo: r".into(), "--@ bogus: x".into()];
        assert!(parse_metadata(&lines).is_err());
    }

    #[test]
    fn empty_required_key_parsed_as_empty_string() {
        let lines = vec!["--@ repo: ".into(), "--@ kind: exec".into()];
        let m = parse_metadata(&lines).unwrap();
        // Empty string — syntax validator will catch this
        assert_eq!(m.repo.as_deref(), Some(""));
    }

    #[test]
    fn dec_split_by_commas() {
        let lines = vec![
            "--@ repo: r".into(),
            "--@ kind: exec".into(),
            "--@ dec: qty, balance_after".into(),
        ];
        let m = parse_metadata(&lines).unwrap();
        assert_eq!(m.dec, vec!["qty", "balance_after"]);
    }

    #[test]
    fn duplicate_keys_error() {
        let lines = vec![
            "--@ repo: r".into(),
            "--@ kind: exec".into(),
            "--@ repo: r2".into(),
        ];
        let err = parse_metadata(&lines).unwrap_err();
        assert!(
            err.to_string().contains("duplicate"),
            "expected 'duplicate' in error: {err}"
        );
    }

    #[test]
    fn unknown_kind_error() {
        let lines = vec!["--@ repo: r".into(), "--@ kind: select".into()];
        let err = parse_metadata(&lines).unwrap_err();
        assert!(err.to_string().contains("unknown kind"));
    }
}
