---
name: testing
description: Test strategies and environments
---

# Testing Strategies

- **Unit Tests**: Place alongside implementation in the same file `mod tests`.
- **Integration Tests**: Place in `tests/` at the workspace root.
- Use `assert_eq!` and `mockall` where appropriate.
- Database tests must use in-memory SQLite modes or isolated tmp directories.
