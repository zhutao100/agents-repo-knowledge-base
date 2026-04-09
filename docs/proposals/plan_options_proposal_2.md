## Reasoning narrative (what I optimized for)

The Harness Engineering approach is directionally correct: keep `AGENTS.md` short as a table-of-contents, keep deeper repo knowledge under `docs/`, and enforce structure/freshness mechanically via linters/CI.
Your failure modes are mostly *addressability* and *determinism* problems: free-text Markdown is expensive to selectively retrieve and risky to patch without reading. The options below all convert “repo map” knowledge into **small, stable, machine-addressable atoms**, backed by **single-call retrieval** and **deterministic freshness gates** that force updates *in-session*.

---

## Design goals that all options satisfy

* **Atomicity + stable IDs:** knowledge is stored as small records with stable keys (not paragraphs in a prose blob).
* **Single-call retrieval:** one tool call yields a *context pack* (or a few records), not “open 10 files”.
* **Two-way links to code:** every fact points to code anchors (path + symbol/range hash), so freshness can be checked mechanically.
* **“No silent drift” gates:** pre-commit and CI fail if required knowledge updates aren’t present.

---

## Option 1 — *Repo Atlas*: module cards + directory manifest (machine-readable “repo map”)

### Core idea & data model

Create a **deterministic, low-churn repo map** as structured data (YAML/TOML/JSON), not prose.

**Artifacts (in-repo):**

* `kb/atlas/modules/*.toml` (or `.yaml`): one “module card” per directory/package.
* `kb/atlas/files.jsonl`: optional per-file intent records (only for key files / entrypoints).
* `kb/atlas/index.json`: derived index for fast lookup (generated).

**Example module card (TOML):**

```toml
id = "payments.core"
path = "src/payments"
purpose = "Payment domain core invariants and orchestrations"
owners = ["team-payments"]
entrypoints = ["src/payments/service.ts:PaymentService"]
edit_points = [
  { capability = "refunds", symbols = ["RefundService", "createRefund"] },
  { capability = "webhooks", symbols = ["WebhookHandler", "verifySignature"] }
]
depends_on = ["common.types", "infra.db"]
```

### Efficient retrieval (minimizing relevance + tool-call churn)

Provide a single CLI (or MCP tool) that returns a **context pack**:

* `kb query capability refunds` → emits the *few* module cards + anchors + recommended files.
* `kb explain path src/payments` → emits module card + top entrypoints + edit points.
* Optional: `kb pack --task "<prompt>"` generates a bounded context bundle (e.g., max N records, max KLOC of code excerpts).

This avoids reading Markdown indexes and avoids N “open file” calls.

### In-session updates

During a task session, the agent:

1. Updates the affected module cards (usually 1–3).
2. Updates only the “edit_points” for the feature being changed (bounded diff).
3. Runs `kb fmt && kb index` to regenerate derived index.

### Freshness enforcement (pre-commit/CI; deterministic)

* **Coverage gate:** every directory matching `src/*` (or every package) must have a module card.
* **Anchor gate:** every `entrypoint`/`edit_point` symbol must resolve (ctags/AST lookup).
* **Index gate:** `kb index` must be up-to-date (CI runs generator and fails on diff).
* **Churn gate:** module cards must remain below size thresholds (prevents prose creep).

### Pros / cons / failure modes

**Pros**

* Extremely practical; low infrastructure.
* Very low relevance churn (records are tiny and addressable).
* Deterministic updates (edit one card, not a narrative doc).

**Cons**

* Requires conventions: what counts as a “module,” what fields are mandatory.
* Cards can still drift if enforcement is weak.

**Expected failure modes**

* “Card sprawl” if you create cards for everything. Mitigate via explicit scope (packages/domains only).
* Agents stuffing prose into `purpose`. Mitigate via field limits + lint.

### Migration from `docs/ + AGENTS.md`

1. Keep `AGENTS.md` as TOC, but add: “Use `kb query/pack` first.”
2. Start with **top-level directories only** (10–30 cards).
3. Gradually enforce coverage (new directories require a card; old ones backfilled over time).

---

## Option 2 — *Atomized Knowledge Graph*: JSONL/SQLite “facts” with code-anchored freshness

### Core idea & data model

Store knowledge as **small fact records** (graph nodes) with stable IDs and typed relations. Use:

* `kb/facts.jsonl` (simple, diff-friendly) **or** `kb/facts.sqlite` (fast query, generated snapshot committed).
* Each fact references **code anchors** (path + symbol + optional hash of AST/range).

**Example fact (JSONL):**

```json
{
  "id": "fact.payment.refund_flow",
  "type": "capability",
  "title": "Refund flow",
  "tags": ["payments", "refunds"],
  "anchors": [
    {"path":"src/payments/refunds.ts","symbol":"createRefund","sig_hash":"..."}
  ],
  "relationships": [
    {"rel":"implemented_by","to":"module.payments.core"},
    {"rel":"validated_by","to":"test.refunds.contract"}
  ],
  "summary": "Refund creation validates eligibility, writes refund row, emits webhook event."
}
```

### Efficient retrieval

Single-call queries against the fact store:

* `kb get fact.payment.refund_flow`
* `kb search tags:refunds`
* `kb pack --task "<prompt>" --topk 25` (returns top facts + their anchors)

Because records are small and typed, agents don’t need to read a whole doc to find “where to edit”.

### In-session updates

Agents add/update facts *as they learn during the task*:

* New capability → add a `capability` fact + anchors.
* Refactor → update anchor references (often automated).
* New invariants → add `invariant` facts linked to modules/tests.

### Freshness enforcement

* **Anchor validity:** CI verifies all anchors resolve and signature hashes match.
* **Fact coverage rules:** e.g., every public API route must have an `api_endpoint` fact; every domain module must link to ≥1 capability fact.
* **Change detection gate:** if files in `src/payments/**` changed, at least one related fact (tagged `payments`) must change, or developer must add an explicit “kb-exempt” rationale in PR metadata.

### Pros / cons / failure modes

**Pros**

* Best for selective retrieval and deterministic patching.
* Powerful for “where should I edit for feature X?” via capability → anchors.
* Supports multiple “views” (capabilities, invariants, APIs, runbooks) without prose docs.

**Cons**

* Needs schema governance to prevent an uncontrolled ontology.
* Teams must decide what minimal “fact types” matter.

**Failure modes**

* Ontology explosion (too many types). Mitigate by starting with ~6–10 types.
* Facts become “mini-docs”. Mitigate with size limits and lint.

### Migration

1. Start by extracting **capabilities + edit points** from existing docs into facts.
2. Keep Markdown as narrative background; treat facts as the primary “lookup layer”.
3. Gradually tighten coverage gates (by domain).

---

## Option 3 — *Generated Structural Index + Context Packs*: code-derived map as primary, human-authored overlays as exceptions

### Core idea & data model

Make the repo map mostly **generated from code**, to minimize human-maintenance drift:

* `kb/gen/symbols.json` (ctags/AST)
* `kb/gen/deps.json` (module/package dependency graph)
* `kb/gen/routes.json` / `kb/gen/schemas.json` (framework-specific extractors)
* `kb/overlays/*.yaml` (small human-authored “why/intent” overlays keyed by stable IDs in the generated outputs)

This aligns with “map, not manual” and “mechanical validation” principles from Harness Engineering.

### Efficient retrieval

Agents never “browse the tree” manually:

* `kb pack --task "<prompt>"` runs:

  1. generate/refresh indexes (fast, local)
  2. retrieve relevant symbols/routes/deps
  3. emit a bounded context pack (JSON + optional curated code excerpts)

This is one call in agent workflows: “run pack, then open only the top-ranked files”.

### In-session updates

Most updates happen automatically by regeneration.
Agents only edit overlays when adding intent that can’t be derived:

* “This module exists because …”
* “Edit points for capability X are intentionally in Y rather than Z.”

### Freshness enforcement

* CI runs `kb gen` and fails on diff (generated files must be checked in and current).
* Overlay lint ensures overlays reference valid generated IDs.
* Optional: “overlay required” policy for new modules above a size threshold (forces minimal intent).

### Pros / cons / failure modes

**Pros**

* Lowest ongoing human/agent maintenance cost.
* Excellent freshness: regeneration is deterministic.
* Strong baseline for navigation (symbols, deps, routes).

**Cons**

* Generated maps answer *what exists*, not always *why it exists*.
* Framework-specific extractors can be non-trivial.

**Failure modes**

* False confidence: agents may treat generated structure as sufficient and miss intent. Mitigate with overlay requirement for key domains.
* Generator becomes slow. Mitigate with incremental generation and caching.

### Migration

1. Start with symbols + deps generation (generic).
2. Add 1–2 domain extractors (routes, schemas) where navigation pain is highest.
3. Gradually move “repo map” content out of prose docs into overlays keyed to generated IDs.

---

## Option 4 — *PR Session Capsules + Compaction*: enforce “in-session capture” as a first-class artifact

### Core idea & data model

Instead of relying on off-session doc gardening, force every meaningful change to emit a **session capsule** that captures:

* task intent, hypotheses tried, key decisions, pitfalls, final edit points, validation steps
* links to touched files/symbols
* auto-extracted diffs metadata (files changed, APIs impacted)

**Artifacts:**

* `kb/sessions/YYYY/MM/<pr-or-task-id>.json`
* Optional compaction output: `kb/facts.jsonl` updates derived from capsules (semi-automated)

### Efficient retrieval

Two retrieval modes:

* **By capability/domain:** query facts (Option 2) *and* pull the most recent capsules for that tag.
* **By file/symbol:** “show last N capsules that touched this symbol.”

This is extremely effective for “day-to-day engineering reality” (pitfalls, why choices were made) that tends to be lost otherwise.

### In-session updates

Agents generate the capsule at the end of the task (or incrementally):

* `kb session start --task "<prompt>"` (creates skeleton)
* `kb session finalize` (fills touched files, test commands executed, outcome)

### Freshness enforcement

* Pre-commit/CI requires:

  * a capsule exists for any PR that changes production code (or above a diff threshold),
  * capsule references at least one verification step (tests, benchmarks, repro),
  * capsule is linked in PR description (or has PR number field).
* Optional: capsule-to-facts compaction must run and be committed when certain tags are present (e.g., `architecture`, `api-change`).

### Pros / cons / failure modes

**Pros**

* Directly solves the “richest context exists during the task session” problem.
* Enables fast recall of pitfalls and rationale without re-deriving.

**Cons**

* Requires discipline; can become bureaucratic if not right-sized.
* Capsules can become verbose if unconstrained.

**Failure modes**

* Low-quality capsules (“did stuff, seems fine”). Mitigate with required fields + lint + size limits, and by auto-filling from repo/test tooling.

### Migration

1. Introduce capsules as optional; auto-generate skeletons.
2. Make capsules mandatory for high-impact changes (API, migrations, architecture).
3. Later, compact capsules into facts (Option 2) to keep retrieval crisp.

---

## Quick comparison

| Option              | Best at                              | Main risk                 | Best enforcement primitive         |
| ------------------- | ------------------------------------ | ------------------------- | ---------------------------------- |
| 1. Repo Atlas       | Deterministic “where do I edit?” map | Cards drift / prose creep | Coverage + anchor-resolution lint  |
| 2. Knowledge Graph  | Selective retrieval + stable atoms   | Ontology sprawl           | Schema + anchor hash checks        |
| 3. Generated Index  | Freshness at scale, low maintenance  | Missing “why”             | Regenerate-and-diff CI gate        |
| 4. Session Capsules | Capturing in-session tacit context   | Bureaucracy / verbosity   | Mandatory capsule per PR threshold |

---

## A pragmatic adoption sequence (lowest risk)

1. **Start with Option 1 (Repo Atlas)** for immediate navigation wins.
2. Add **Option 3 (Generated Index)** to make freshness cheap and deterministic.
3. Layer **Option 4 (Session Capsules)** to eliminate “lost in-session context.”
4. When retrieval needs become more complex, introduce **Option 2 (Knowledge Graph)** as the canonical “lookup layer” and optionally compact capsules into facts.

This preserves the Harness Engineering “small entry point + progressive disclosure + mechanical validation” pattern  while addressing your specific failure modes: deterministic repo mapping, machine-addressable retrieval, and in-session freshness enforcement.
