---
name: security
description: Risk tiers and credentials handling
---

# Security Policies

- **Tier 1 (Safe)**: Non-destructive read ops. Automatic execution permitted.
- **Tier 2 (Sensitive)**: Writing files, simple network requests. Requires explicit config enable.
- **Tier 3 (Dangerous)**: Native terminal execution, modifying config. Requires confirmation.

Never log raw API keys. Always use `SecretString` from `SecretCache`.
