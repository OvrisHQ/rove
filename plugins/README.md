# Rove Plugins

WASM plugins built with the Extism SDK, sandboxed and hot-loadable at runtime.

- **fs-read** -- Filesystem reading
- **fs-editor** -- Filesystem editing
- **terminal** -- Command execution
- **git** -- Git operations
- **screenshot** -- Screen capture

Build a plugin with:

```sh
cargo build --target wasm32-wasip1 -p <plugin-name>
```
