# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace for a modular ERP monolith. Core code lives under `crates/`: `kernel` holds shared contracts and types, infrastructure crates such as `db`, `auth`, `audit`, and `event_bus` provide platform services, `warehouse` is the current domain module, and `gateway` exposes the HTTP entry point as the `erp-gateway` binary. Database migrations live in `migrations/common` and `migrations/warehouse`. UI assets are under `web/static` and `web/templates`. Planning and design notes live in `docs/` and `doc/`.

## Build, Test, and Development Commands

Use `just` recipes as the default workflow:

- `just build` builds the full workspace.
- `just test` runs all workspace tests.
- `just test-crate warehouse` runs one crate’s tests.
- `just fmt` and `just fmt-check` apply or verify formatting.
- `just lint` runs `clippy` with warnings denied.
- `just check` runs formatting, linting, dependency policy, and tests.
- `just run` starts the gateway locally; `just run-debug` enables `RUST_LOG=debug`.
- `just db-migrate` applies migrations from `migrations/common`.

Copy `.env.example` to `.env` before running database-backed commands.

## Coding Style & Naming Conventions

Target the stable toolchain defined in `rust-toolchain.toml`; `rustfmt` and `clippy` are required components. Formatting uses 4-space indentation, `max_width = 100`, and field init shorthand. Follow Rust defaults: `snake_case` for modules/functions, `PascalCase` for types/traits, and `SCREAMING_SNAKE_CASE` for constants. Prefer small crates with explicit boundaries and keep shared abstractions in `kernel`, not in domain crates.

## Testing Guidelines

Use `cargo test` through `just test` for all changes. Integration and end-to-end suites belong in `tests/integration` and `tests/e2e`; crate-local unit tests should stay next to the code they cover. Name tests for observable behavior, for example `creates_sequence_without_gaps`. Add tests for new business rules, migrations, and bug fixes before opening a PR.
For BC integration tests in this repository, follow [`docs/testing_integration_style.md`](/home/raa/RustProjects/erp/docs/testing_integration_style.md): reuse `crates/test_support` for shared pool, request context, pipeline wiring, and tenant cleanup instead of duplicating setup code per crate.
For API regression, Postman/Newman structure, and Definition of Done for new BC endpoints, follow [`docs/api_testing_rules.md`](/home/raa/RustProjects/erp/docs/api_testing_rules.md).

## Commit & Pull Request Guidelines

Current history uses short, imperative subjects with a scope prefix, for example `Layer 0: Cargo workspace + toolchain + infrastructure scaffold`. Keep commits focused and describe the architectural layer or module first. PRs should explain the change, list affected crates, note required env or migration steps, and include sample requests/responses when `gateway` behavior changes.
