//! `repo-gen` — generates ergonomic BC persistence API on top of clorinde.
//!
//! Usage:
//!   repo-gen --all
//!   repo-gen --bc warehouse

mod clorinde_parser;
mod emit_facade;
mod emit_repo;
mod emit_types;
mod model;
mod sql_parser;
mod validator;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::model::ResolvedQuery;

#[derive(Parser)]
#[command(
    name = "repo-gen",
    about = "Generate BC persistence API from SQL + metadata"
)]
struct Cli {
    /// Generate for all BCs (excluding common/).
    #[arg(long, conflicts_with = "bc")]
    all: bool,

    /// Generate for a specific BC.
    #[arg(long)]
    bc: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let project_root = find_project_root()?;
    let queries_dir = project_root.join("queries");
    let clorinde_dir = project_root.join("crates/clorinde-gen/src/queries");
    let crates_dir = project_root.join("crates");

    let bc_names = discover_bcs(&queries_dir, &cli)?;
    if bc_names.is_empty() {
        bail!("No BCs found in {}", queries_dir.display());
    }

    eprintln!("repo-gen: processing BCs: {:?}", bc_names);

    for bc in &bc_names {
        let bc_queries_dir = queries_dir.join(bc);
        let bc_clorinde_dir = clorinde_dir.join(bc);
        let bc_crate_dir = crates_dir.join(bc);

        // Phase 1: parse SQL blocks + syntax validation (metadata only)
        let blocks = sql_parser::parse_bc_sql_files(&bc_queries_dir)
            .with_context(|| format!("parsing SQL for BC '{bc}'"))?;

        for block in &blocks {
            validator::validate_syntax(block)
                .with_context(|| format!("{}/{} :: {}", bc, block.source_file, block.name))?;
        }

        eprintln!("  {bc}: parsed {} queries, syntax valid", blocks.len());

        // Phase 2: clorinde introspection + semantic validation
        let clorinde_info = clorinde_parser::parse_bc_clorinde(&bc_clorinde_dir, &blocks)
            .with_context(|| format!("parsing clorinde-gen for BC '{bc}'"))?;

        for block in &blocks {
            let info = clorinde_info
                .get(&block.name)
                .ok_or_else(|| anyhow::anyhow!("no clorinde info for query '{}'", block.name))?;
            validator::validate_semantic(block, info)
                .with_context(|| format!("{}/{} :: {}", bc, block.source_file, block.name))?;
        }

        eprintln!("  {bc}: semantic validation passed");

        // Phase 3: build resolved model
        let resolved = model::resolve(bc, &blocks, &clorinde_info)
            .with_context(|| format!("resolving model for BC '{bc}'"))?;

        eprintln!("  {bc}: resolved {} queries", resolved.len());

        // Step 5: emit generated code
        let out_dir = bc_crate_dir.join("src/db/generated");
        prepare_out_dir(&out_dir)?;

        emit_code(bc, &resolved, &out_dir)?;

        eprintln!("  {bc}: generated code written to {}", out_dir.display());
    }

    eprintln!("repo-gen: done. Run `cargo fmt --all` to format.");
    Ok(())
}

fn prepare_out_dir(out_dir: &Path) -> Result<()> {
    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir)
            .with_context(|| format!("removing stale {}", out_dir.display()))?;
    }
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    Ok(())
}

fn emit_code(bc: &str, queries: &[ResolvedQuery], out_dir: &Path) -> Result<()> {
    // Group by repo
    let mut by_repo: BTreeMap<String, Vec<&ResolvedQuery>> = BTreeMap::new();
    for q in queries {
        by_repo.entry(q.repo_group.clone()).or_default().push(q);
    }

    // Emit types.rs
    let types_code = emit_types::emit(bc, queries);
    std::fs::write(out_dir.join("types.rs"), types_code)?;

    // Emit per-repo files
    let mut repo_names: BTreeSet<String> = BTreeSet::new();
    for (repo_name, repo_queries) in &by_repo {
        let code = emit_repo::emit(bc, repo_name, repo_queries);
        std::fs::write(out_dir.join(format!("{repo_name}.rs")), code)?;
        repo_names.insert(repo_name.clone());
    }

    // Emit mod.rs (facade)
    let mod_code = emit_facade::emit(bc, &repo_names);
    std::fs::write(out_dir.join("mod.rs"), mod_code)?;

    Ok(())
}

fn find_project_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("queries").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("Cannot find project root (looking for Cargo.toml + queries/)");
        }
    }
}

fn discover_bcs(queries_dir: &Path, cli: &Cli) -> Result<Vec<String>> {
    if let Some(bc) = &cli.bc {
        let bc_dir = queries_dir.join(bc);
        if !bc_dir.is_dir() {
            bail!("BC directory not found: {}", bc_dir.display());
        }
        return Ok(vec![bc.clone()]);
    }

    // --all: find all subdirectories except "common"
    let mut bcs = Vec::new();
    for entry in std::fs::read_dir(queries_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type()?.is_dir() && name != "common" {
            bcs.push(name);
        }
    }
    bcs.sort();
    Ok(bcs)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::prepare_out_dir;

    #[test]
    fn prepare_out_dir_removes_stale_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("repo_gen_prepare_out_dir_{unique}"));
        let out_dir = root.join("generated");

        std::fs::create_dir_all(&out_dir).expect("create initial dir");
        std::fs::write(out_dir.join("stale.rs"), "// stale").expect("write stale file");

        prepare_out_dir(&out_dir).expect("prepare out dir");

        assert!(out_dir.exists());
        assert!(!out_dir.join("stale.rs").exists());

        std::fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
