# Repository Guidelines

## Project Structure & Module Organization
- `controller/` hosts lending logic with integration suites under `controller/tests/`.
- `liquidity_layer/` manages pool accounting; `price_aggregator/` aggregates feeds; mocks live in `flash_mock/` and `swap_mock/`.
- Shared crates reside in `common/*`; update them before duplicating structs, math, or errors.
- Tooling sits in `configs/` with `deploy-*.json` and `script.sh`; artifacts emit to `*/output/` and `output-docker/`.
- Proxies in `proxy/` and `common/proxies` are auto-generated; ignore manual edits and regenerate via the build pipeline.

## Build, Test, and Development Commands
- `make build` – reproducible WASM builds inside Docker.
- `cargo test` – full workspace run; narrow with filters like `cargo test liquidations`.
- `cargo fmt --all && cargo clippy --workspace -- -D warnings` – mandatory hygiene before commits.
- `make devnet <action>` / `make mainnet <action>` – scripted flows via `configs/script.sh`; agents never deploy—hand configs to protocol ops.

## Coding Style & Naming Conventions
- Run `cargo fmt --all`; `rustfmt.toml` enforces 4-space indents, 100-character lines, and trailing commas.
- Follow idiomatic casing (`snake_case` modules, `CamelCase` types, `SCREAMING_SNAKE_CASE` constants) and keep `src/lib.rs` entrypoints thin.
- Reuse enums from `common/errors`, guard state with `require!`, and add `///` docs for non-obvious parameters.

## Testing Guidelines
- Co-locate tests with their code (e.g., `controller/tests/borrow.rs`, `common/math/tests/`) and reuse fixtures from `controller/tests/setup/`.
- Run `cargo test` plus the suites you touched; hold coverage near ~90% and refresh `coverage.md` when risk flows move.
- For script or CLI tweaks, dry-run against mocks (`make devnet <command>`) and note key diffs in the PR.

## Security, Pitfalls & Math Validation
- Consult `ORACLE.md` before feed or tolerance changes and keep `networks.json` aligned with deployments.
- Recheck invariants—health factor ≥ 1, liquidity conservation, borrow caps, pause flags, access control—and add regression tests when adjusting guards.
- Validate finance math: RAY/WAD/BPS scaling, interest slopes, Taylor approximations, and rounding edges; share numeric examples for new formulas.
- Prefer templated JSON in `configs/` for new markets and document rollback steps in PRs.

## Commit & Pull Request Guidelines
- Use short, present-tense commits; prefixes such as `feat:` or `fix:` match existing history.
- Before pushing, confirm `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, and `make build` succeed; attach logs or screenshots for behavior changes.
- PRs must summarize impact, list affected markets/configs, link issues, and flag migrations, security notes, and manual test results. Follow `SECURITY.md` and keep secrets out of version control.
