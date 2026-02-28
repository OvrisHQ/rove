---
name: brains
description: Local inference rules and configuration
---

# Brains Configuration

The Brains system manages local model execution:

- Local models require RAM and should only be loaded if `[brains]` is enabled in config.
- Rank providers utilizing local models when task sensitivity > threshold (e.g. 0.8).
- Unload models when idle if `auto_unload` is enabled.
