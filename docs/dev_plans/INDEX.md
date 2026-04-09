# kb-tool dev plans

This directory contains **indexed, decision-complete** development plans for implementing the kb-tool baseline described in:

* `docs/DESIGN.md`
* `docs/SPECS.md`

Plans are written to be implemented without additional design decisions. If a plan introduces or changes any user-facing CLI contract, artifact schema, or enforcement rule, it must be reconciled with `docs/DESIGN.md` and `docs/SPECS.md` in the same change.

---

## Conventions

* Plan IDs are `DP-000N` and map to a file `000N-<slug>.md`.
* Plans assume a Rust single-binary implementation unless explicitly called out.
* All outputs must remain typed, local, repo-bounded, and deterministic.

---

## Plan index

| Plan ID   | File                             | Goal |
| --------- | -------------------------------- | ---- |
| DP-0001   | `docs/dev_plans/0001-cli-and-io.md` | CLI skeleton, canonical IO, diff-source reader |
| DP-0002   | `docs/dev_plans/0002-index-gen.md` | Deterministic index generation + `index check` |
| DP-0003   | `docs/dev_plans/0003-pack-and-plan.md` | Deterministic `plan diff` + `pack` planners |
| DP-0004   | `docs/dev_plans/0004-enforcement.md` | Pre-commit/CI gates and target-repo integration |
| DP-0005   | `docs/dev_plans/0005-describe-and-list.md` | Typed discovery (`list`) + deterministic lookups (`describe`) |
| DP-0006   | `docs/dev_plans/0006-sessions.md` | Session capsules (`kb session *`) and templates |
