---
name: rust_idioms
description: Idiomatic Rust practices
---

# Rust Idioms

- Use `anyhow::Result` for application-level errors.
- Avoid `.unwrap()` and `.expect()` in production paths. Propagate with `?`.
- Prefer exhaustive matches on enums.
- Use `tracing` crate `info!`, `warn!`, `debug!`, `error!` instead of `println!`.
