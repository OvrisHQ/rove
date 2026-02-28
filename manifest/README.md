# Manifest & Code Signing System

Rove uses Ed25519 key-based signing to verify the integrity of binaries and plugins distributed through GitHub Releases. The public key is embedded into the engine at compile time; the private key lives only in GitHub Secrets.

## How It Works

```
┌─────────────┐    compile-time     ┌──────────────────┐
│ Public Key   │ ──────────────────► │ Engine Binary     │
│ (32 bytes)   │   via build.rs     │ (verifies sigs)   │
└─────────────┘                     └──────────────────┘

┌─────────────┐    CI release job   ┌──────────────────┐
│ Private Key  │ ──────────────────► │ Signed Manifest   │
│ (GitHub Secret) sign-manifest.py  │ (manifest.json)   │
└─────────────┘                     └──────────────────┘
```

1. `build.rs` reads the public key and embeds it into every compiled binary
2. During a release (`git tag v*` + push), CI builds binaries with the real public key
3. `build-manifest.py` hashes all release artifacts into `manifest.json`
4. `sign-manifest.py` signs the manifest with the private key
5. At runtime, Rove verifies downloaded updates against the embedded public key

## Files in This Directory

| File | Committed | Purpose |
|------|-----------|---------|
| `team_public_key.bin` | Yes | 32-byte raw Ed25519 public key |
| `team_public_key.hex` | Yes | Same key, hex-encoded (human-readable) |
| `dev_public_key.bin` | Yes | Development-only key for local testing |
| `build-manifest.py` | Yes | Generates `manifest.json` with SHA-256 hashes of all artifacts |
| `sign-manifest.py` | Yes | Signs `manifest.json` using the private key |
| `gen_dev_key.sh` | Yes | Helper to generate a dev keypair locally |
| `manifest.json` | No (gitignored) | Generated manifest — only created during release |

## Key Loading Priority (build.rs)

At compile time, `engine/build.rs` looks for the public key in this order:

1. **`ROVE_TEAM_PUBLIC_KEY` env var** (hex-encoded, 64 hex chars = 32 bytes)
2. **`manifest/team_public_key.bin`** (raw 32 bytes)
3. **`manifest/team_public_key.hex`** (hex string)
4. **Placeholder** (32 zero bytes) — development fallback, triggers a build warning

In CI release builds, the env var is set from GitHub Secrets so the real key is always used.

## GitHub Secrets Setup

Two secrets must be configured at **Settings > Secrets and variables > Actions**:

| Secret Name | Value | Used By |
|-------------|-------|---------|
| `ROVE_TEAM_PUBLIC_KEY` | `a980f9...4f179a` (64 hex chars) | `release.yml` build step — passed as env var to `build.rs` |
| `ROVE_TEAM_PRIVATE_KEY` | Full PEM contents of private key | `release.yml` signing step — used by `sign-manifest.py` |

## For New Developers

You do **not** need the real keys for local development. The build system automatically falls back to a placeholder key and prints a warning:

```
warning: No team public key found, generating placeholder
warning: Using placeholder team public key for development
```

This is expected and harmless. All features work locally with the placeholder.

### If you need a local dev keypair

```bash
# Generate a dev Ed25519 keypair
bash manifest/gen_dev_key.sh

# Or manually with openssl:
openssl genpkey -algorithm Ed25519 -out /tmp/rove_dev_private.pem
openssl pkey -in /tmp/rove_dev_private.pem -pubout -outform DER | tail -c 32 > manifest/dev_public_key.bin
```

### Building with a specific key locally

```bash
ROVE_TEAM_PUBLIC_KEY=a980f98925a7e93f44f5afa927e6b7ab915f55001447acc279d4a5eac54f179a \
  cargo build -p engine
```

## Generating a New Team Keypair

If the team key needs to be rotated:

```bash
# 1. Generate new Ed25519 keypair
openssl genpkey -algorithm Ed25519 -out team_private.pem

# 2. Extract 32-byte raw public key
openssl pkey -in team_private.pem -pubout -outform DER | tail -c 32 > manifest/team_public_key.bin

# 3. Create hex version
xxd -p manifest/team_public_key.bin | tr -d '\n' > manifest/team_public_key.hex

# 4. Update GitHub Secrets with:
#    ROVE_TEAM_PUBLIC_KEY  = contents of team_public_key.hex
#    ROVE_TEAM_PRIVATE_KEY = contents of team_private.pem

# 5. Delete the local private key — it must only live in GitHub Secrets
rm team_private.pem
```

## Security Rules

- The private key must **never** be committed to git
- The private key lives **only** in GitHub Secrets (`ROVE_TEAM_PRIVATE_KEY`)
- Public keys (`team_public_key.bin`, `.hex`) are safe to commit
- After key rotation, all previously signed manifests become invalid
- Users running `rove update` will verify downloads against the key embedded in their current binary
