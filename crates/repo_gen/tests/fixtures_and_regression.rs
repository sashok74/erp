//! Fixture/golden tests and regression tests for repo-gen.

use std::path::PathBuf;

fn project_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop(); // crates/
    dir.pop(); // project root
    dir
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

// ══════════════════════════════════════════════════════════════════════════
// Fixture helpers — inline parsing (repo_gen is a binary crate)
// ══════════════════════════════════════════════════════════════════════════

mod helpers {
    use std::collections::HashMap;
    use std::path::Path;

    use anyhow::{Result, bail};
    use heck::ToUpperCamelCase;

    // ── Minimal SQL parser ──

    #[derive(Debug)]
    pub struct Block {
        pub name: String,
        #[allow(dead_code)]
        pub kind: String,
        #[allow(dead_code)]
        pub dec: Vec<String>,
        pub file_stem: String,
    }

    pub fn parse_sql_blocks(queries_dir: &Path) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(queries_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "sql"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let content = std::fs::read_to_string(&path)?;
            let file_stem = path.file_stem().unwrap().to_string_lossy().to_string();

            let mut current_name: Option<String> = None;
            let mut current_kind = String::new();
            let mut current_dec = Vec::new();

            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(name) = trimmed.strip_prefix("--!") {
                    if let Some(prev) = current_name.take() {
                        blocks.push(Block {
                            name: prev,
                            kind: std::mem::take(&mut current_kind),
                            dec: std::mem::take(&mut current_dec),
                            file_stem: file_stem.clone(),
                        });
                    }
                    let raw = name.trim();
                    current_name = Some(raw.split_whitespace().next().unwrap_or(raw).to_string());
                } else if let Some(body) = trimmed.strip_prefix("--@") {
                    let body = body.trim();
                    if let Some((key, val)) = body.split_once(':') {
                        match key.trim() {
                            "kind" => current_kind = val.trim().to_string(),
                            "dec" => {
                                current_dec = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some(name) = current_name.take() {
                blocks.push(Block {
                    name,
                    kind: current_kind,
                    dec: current_dec,
                    file_stem,
                });
            }
        }
        Ok(blocks)
    }

    // ── Minimal clorinde parser ──

    #[derive(Debug)]
    pub struct QueryInfo {
        pub all_params: Vec<String>,
        pub params_without_tenant: Vec<String>,
        pub row_fields: Vec<String>,
    }

    pub fn parse_clorinde_info(
        clorinde_dir: &Path,
        blocks: &[Block],
    ) -> Result<HashMap<String, QueryInfo>> {
        let mut by_file: HashMap<String, Vec<&Block>> = HashMap::new();
        for b in blocks {
            by_file.entry(b.file_stem.clone()).or_default().push(b);
        }

        let mut result = HashMap::new();
        for (stem, file_blocks) in &by_file {
            let rs = clorinde_dir.join(format!("{stem}.rs"));
            let source = std::fs::read_to_string(&rs)?;
            let syntax = syn::parse_file(&source)?;

            for block in file_blocks {
                let camel = block.name.to_upper_camel_case();
                let stmt = format!("{camel}Stmt");

                let all_params = extract_bind_names(&syntax, &stmt)?;
                let params_without_tenant: Vec<String> = all_params
                    .iter()
                    .filter(|p| *p != "tenant_id")
                    .cloned()
                    .collect();
                let row_fields = extract_row_field_names(&syntax, &camel);

                result.insert(
                    block.name.clone(),
                    QueryInfo {
                        all_params,
                        params_without_tenant,
                        row_fields,
                    },
                );
            }
        }
        Ok(result)
    }

    fn extract_bind_names(file: &syn::File, stmt_name: &str) -> Result<Vec<String>> {
        for item in &file.items {
            let syn::Item::Impl(imp) = item else { continue };
            let syn::Type::Path(tp) = imp.self_ty.as_ref() else {
                continue;
            };
            if tp
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .as_deref()
                != Some(stmt_name)
            {
                continue;
            }
            for item in &imp.items {
                let syn::ImplItem::Fn(m) = item else { continue };
                if m.sig.ident != "bind" {
                    continue;
                }
                let mut names = Vec::new();
                for input in &m.sig.inputs {
                    let syn::FnArg::Typed(pt) = input else {
                        continue;
                    };
                    if let syn::Pat::Ident(i) = pt.pat.as_ref() {
                        let n = i.ident.to_string();
                        if n != "client" {
                            names.push(n);
                        }
                    }
                }
                return Ok(names);
            }
        }
        bail!("bind not found in {stmt_name}")
    }

    fn extract_row_field_names(file: &syn::File, camel: &str) -> Vec<String> {
        for item in &file.items {
            let syn::Item::Struct(s) = item else { continue };
            if s.ident != camel {
                continue;
            }
            let syn::Fields::Named(f) = &s.fields else {
                continue;
            };
            return f
                .named
                .iter()
                .filter_map(|f| Some(f.ident.as_ref()?.to_string()))
                .collect();
        }
        Vec::new()
    }
}

// ══════════════════════════════════════════════════════════════════════════
// Fixture: simple_lookup
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn fixture_simple_lookup() {
    let dir = fixtures_dir().join("simple_lookup");
    let blocks = helpers::parse_sql_blocks(&dir.join("queries")).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].name, "find_by_sku");
    assert_eq!(blocks[1].name, "create_item");

    let info = helpers::parse_clorinde_info(&dir.join("clorinde"), &blocks).unwrap();

    let find = info.get("find_by_sku").unwrap();
    assert!(find.all_params.contains(&"tenant_id".to_string()));
    assert_eq!(find.all_params[0], "tenant_id", "tenant_id must be first");
    assert_eq!(find.row_fields, vec!["id", "sku", "name"]);

    let create = info.get("create_item").unwrap();
    assert!(create.row_fields.is_empty());
    assert_eq!(create.params_without_tenant, vec!["id", "sku", "name"]);
}

// ══════════════════════════════════════════════════════════════════════════
// Fixture: decimal_write_and_read
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn fixture_decimal_write_and_read() {
    let dir = fixtures_dir().join("decimal_write_and_read");
    let blocks = helpers::parse_sql_blocks(&dir.join("queries")).unwrap();
    assert_eq!(blocks.len(), 2);

    let info = helpers::parse_clorinde_info(&dir.join("clorinde"), &blocks).unwrap();

    // upsert: dec=balance must be in bind params
    let upsert = info.get("upsert_balance").unwrap();
    assert!(upsert.all_params.contains(&"balance".to_string()));

    // get: dec=balance must be in row fields
    let get = info.get("get_balance").unwrap();
    assert!(get.row_fields.contains(&"balance".to_string()));
}

// ══════════════════════════════════════════════════════════════════════════
// Regression: actual project
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn regression_repo_gen_succeeds_on_real_project() {
    let root = project_root();
    let output = std::process::Command::new("cargo")
        .args(["run", "-p", "repo_gen", "--", "--all"])
        .current_dir(&root)
        .output()
        .expect("failed to run repo-gen");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "repo-gen failed:\n{stderr}");
    assert!(stderr.contains("warehouse"), "missing warehouse in output");
    assert!(stderr.contains("catalog"), "missing catalog in output");
}

#[test]
fn regression_workspace_compiles_after_generation() {
    let root = project_root();
    let output = std::process::Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo check");

    assert!(
        output.status.success(),
        "cargo check failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
