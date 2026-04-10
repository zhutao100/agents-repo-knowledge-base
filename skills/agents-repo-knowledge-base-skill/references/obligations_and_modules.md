# Obligations and modules (design guidance)

## Principles

- Obligations are **deterministic** and **prefix-based**.
- Avoid heuristic triggers, fuzzy search, or free-text “task/question” fields.
- Start with **no rules**, then add narrow rules as the repo map matures.

## Minimal obligations.toml

The file is required by `kb lint all`, but it may contain zero rules.

Recommended first rule patterns:

- “If anything under `src/api/` changes, update module card `api.core`.”
- “If anything under `migrations/` changes, update facts of type `data_migration` and add a session capsule.”

## Module cards

Module cards live at:

- `kb/atlas/modules/<MODULE_ID>.toml`

Constraints:

- `id` must equal the filename stem.
- Prefer a small number of modules with strong entrypoints/edit_points.

Template:

```toml
id = "<module.id>"
title = "<Human title>"
tags = []
entrypoints = ["<path>"]
edit_points = ["<path>"]
related_facts = []
```

## Session capsules

Use session capsules for context that is not reliably rediscoverable off-session (decisions, pitfalls, verification).

- `kb session init --id <ID> [--tag <TAG>]...`
- Edit the created JSON.
- `kb session finalize --id <ID> --diff-source staged --verification tests --verification lint`
