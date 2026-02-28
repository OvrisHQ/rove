# Rove Security Documentation

## Risk Tier System

All operations are classified into three risk tiers:

| Tier | Description | Examples | Controls |
|------|-------------|----------|----------|
| **0** | Read-only | read_file, list_dir, file_exists | No confirmation |
| **1** | Local modifications | write_file, run_command (safe) | Countdown confirmation (configurable delay) |
| **2** | Destructive/remote | git push, rm -rf, --force flags | Explicit confirmation required |

### Dangerous Flag Escalation
Commands with these flags are escalated to Tier 2:
`--force`, `-rf`, `--delete`, `--hard`, `--no-verify`

### Remote Operation Escalation
Operations from remote sources (Telegram) are escalated one tier.

## File System Security (FileSystemGuard)

### Four Checks
1. **Pre-canonicalization deny list** - Check path components against deny list
2. **Canonicalization** - Resolve symlinks and `..` patterns
3. **Post-canonicalization deny list** - Re-check after resolution
4. **Workspace boundary** - Verify path is within configured workspace

### Deny List
Sensitive paths blocked: `.ssh`, `.env`, `credentials`, `.aws`, `.gnupg`, `id_rsa`, `id_ed25519`, `.keychain`, etc.

### Path Traversal Prevention
- Double canonicalization prevents symlink bypass
- `..` components resolved before boundary check
- URL-encoded traversal patterns detected

## Command Execution Security (CommandExecutor)

### Allowlist Validation
Only approved commands can be executed. Shell patterns rejected:
- `sh -c`, `bash -c` patterns
- Shell metacharacters: `|`, `&`, `;`, `` ` ``, `$()`, `>`, `<`
- Dangerous piping patterns

### Execution Model
- No shell interpretation (execve-style)
- stdin set to null
- stdout/stderr piped and captured
- 60-second timeout

## Injection Detection (InjectionDetector)

Regex patterns detect common prompt injection phrases:
- "ignore previous instructions"
- "forget your instructions"
- "new instructions"
- "override system prompt"
- "disregard" patterns

Detected injections are logged and blocked.

## Secret Management

### Storage
- OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- Never stored in files or environment variables
- Interactive prompting with immediate keychain storage

### Scrubbing
All output is scrubbed for secret patterns:
- OpenAI keys: `sk-[a-zA-Z0-9]{20,}`
- Google keys: `AIza[0-9A-Za-z-_]{35}`
- Telegram tokens: `[0-9]{10}:[a-zA-Z0-9-_]{35}`
- GitHub tokens: `ghp_[a-zA-Z0-9]{36}`
- Bearer tokens: `Bearer\s+[^\s]{20,}`

## Cryptographic Operations

### Ed25519 Signatures
- Team public key embedded at compile time
- Manifest signed with team private key (kept offline)
- Individual tool signatures verified at load time

### BLAKE3 Hashing
- File integrity verification
- Compromised files deleted immediately on hash mismatch

### Envelope Verification
- Timestamp validation (30-second window)
- Nonce cache prevents replay attacks
- Nonces evicted after 30 seconds

## Rate Limiting

| Scope | Limit | Window |
|-------|-------|--------|
| General | 60 ops | 1 hour |
| Tier 2 | 10 ops | 10 minutes |
| Tier 2 burst | 5 ops | 60 seconds |

Circuit breaker activates for Tier 2 after sustained rate limit hits.

## Native Runtime (4-Gate Verification)

1. **Gate 1**: Tool declared in signed manifest
2. **Gate 2**: BLAKE3 hash matches manifest entry
3. **Gate 3**: Team Ed25519 signature on manifest
4. **Gate 4**: Individual tool Ed25519 signature

Failure at any gate → immediate file deletion.

## WASM Runtime (2-Gate Verification)

1. **Gate 1**: Plugin declared in manifest
2. **Gate 2**: BLAKE3 hash matches manifest entry

Additional protections:
- Crash isolation (restart without engine impact)
- Host function mediation for all file/command access
- No direct message bus access
