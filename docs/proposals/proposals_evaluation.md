## 1) Problem restatement (what must be solved)

You are targeting the specific gap where agents struggle relative to experienced engineers: *cheap, reliable repo navigation and selective context retrieval* under (a) no cross-session memory and (b) a fixed context window. You also call out two concrete churn types—**relevance churn** (dumping large markdown/code to find small facts) and **IO churn** (lots of small tool calls to traverse trees/files)—and you require **commit-gated** enforcement that forces knowledge updates *in-session* (not deferred “doc gardening”).

The Harness Engineering pattern (short `AGENTS.md` as TOC + structured `docs/` as system of record + mechanical checks + doc-gardening) is directionally aligned, but the proposals correctly highlight what remains weak: directory/file-level “map memory,” selective read/write, and capturing session-only rationale.

---

## 2) How the two proposals relate (they largely converge)

The proposals are complementary, not competing:

* **Proposal 1** frames a “KB tool” around a *generated* repo graph (tree/symbols/deps/obligations) plus optional manifests, verifiable assertions, and structured worklogs.
* **Proposal 2** expands the design space into four options (Repo Atlas cards, Knowledge Graph facts, Generated Index + overlays, Session Capsules) and provides an adoption sequence.

A practical mapping:

| Proposal 1                                  | Proposal 2                                 | Relationship                                                                                                                    |
| ------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Generated repo graph + single KB query tool | Generated structural index + context packs | Same “generated-first navigation substrate”; Proposal 1 adds **obligations** + diff-driven gating as a first-class primitive.   |
| Co-located module manifests                 | Repo Atlas module cards                    | Same “human-authored module summaries”; Proposal 2 gives more concrete “edit points” and card governance.                       |
| Worklog JSONL                               | Session capsules                           | Same goal: capture in-session rationale/pitfalls in a structured way.                                                           |
| Verifiable assertions in docs               | (Not explicit in P2)                       | A useful add-on that turns prose drift into a failing invariant.                                                                |

---

## 3) Evaluation criteria (grounded in your requirements)

From your task doc, the options should be evaluated on:

1. **Navigation efficiency:** does it encode b/c navigation as cheap, queryable memory (directory/file/symbol “map”)?
2. **Selective retrieval:** can agents answer “where do I edit?” without full-doc reads?
3. **Low IO churn:** can typical “map queries” be answered with *one* tool invocation?
4. **Commit-gated freshness:** can the system deterministically block merges when required knowledge isn’t updated?
5. **In-session capture:** does it preserve “why-not-X / pitfalls / experiments” that doc-gardening misses?
6. **Operational complexity:** generator/ontology/tooling burden; likelihood of accidental bureaucracy.

These are explicitly emphasized across the background and both proposals.

---

## 4) Option-by-option evaluation

### Option A — Generated structural index + single “KB query/pack” tool (P1 Option 1; P2 Option 3)

**What it is:** Generate repo tree/symbols/deps (and optionally routes/schemas) and expose a single CLI/MCP command that returns a bounded “context pack.” Proposal 1 adds **obligations** and **plan-from-diff** to compute “what knowledge must change” from a diff.

**Strengths**

* Best match for **relevance churn** + **IO churn**: one tool call returns exactly the slice needed (directory digest, symbol defs/uses, entry points).
* Best match for **commit-gated freshness** when combined with deterministic regeneration and “diff ⇒ obligations” checks.
* Lowest long-run drift risk because the “map” is derived from code, not maintained by hand.

**Weaknesses / risks**

* Generated maps answer “what exists” better than “why it exists”; you will need overlays/cards/capsules for intent and rationale.
* Generator performance and determinism become platform-critical (sort stability, incremental regen, reproducible symbol extraction).

**Verdict:** This is the *core substrate* that directly attacks your biggest gap (cheap navigation) while naturally enabling commit gating.

---

### Option B — Co-located module manifests / Repo Atlas cards (P1 Option 2; P2 Option 1)

**What it is:** Small structured “module cards” (YAML/TOML/etc.) per directory/package with purpose, entry points, and “edit points”; optionally a repo-wide index generated from these cards.

**Strengths**

* Captures the human-style navigation affordance you describe (“this directory is X; edit points are Y”).
* Localized updates reduce mega-doc drift (edit one card, not a sprawling doc).
* Pairs well with commit gating: “changes under path ⇒ card must be updated or regenerated.”

**Weaknesses / risks**

* If cards are *primary* (not overlays), they can still drift and/or grow into prose unless aggressively linted (size thresholds, mandatory fields, anchor resolution).
* If you have many modules, “card coverage” can become a migration tax unless bootstrapped and enforced gradually.

**Verdict:** Best treated as a **thin “why/intent” overlay** on top of the generated index, not as the sole map.

---

### Option C — Atomized Knowledge Graph facts (P2 Option 2)

**What it is:** JSONL/SQLite “facts” with stable IDs, types (capability/invariant/api endpoint/etc.), and code anchors (path+symbol+hash), queried via a single command.

**Strengths**

* Strongest **selective retrieval** and “semantic lookup” once mature (“capability refunds ⇒ anchors ⇒ tests ⇒ invariants”).
* Strong mechanical freshness story if anchors are validated and hashes checked in CI.
* Enables multiple “views” without rewriting prose (capabilities, runbooks, invariants).

**Weaknesses / risks**

* High governance burden: ontology sprawl and “facts becoming mini-docs” are real failure modes unless you constrain types and size.
* If introduced too early, it risks becoming a parallel documentation universe competing with other artifacts.

**Verdict:** High ceiling, higher complexity. Best introduced **after** you have a working generated index + enforcement loop, or introduced narrowly (few fact types, few domains).

---

### Option D — Verifiable assertions embedded in docs (P1 Option 3)

**What it is:** Keep markdown, but embed structured assertion blocks that a linter can verify against code/index (endpoints exist, config keys supported, dependency rules, ownership).

**Strengths**

* Converts doc drift into a hard failing invariant (very aligned with “mechanical enforcement” ethos from Harness Engineering).
* Works well as an incremental retrofit: start with a small number of high-value assertions.

**Weaknesses / risks**

* Does not directly solve navigation/IO churn unless paired with the KB query tool.
* Linter authoring cost can balloon if you try to “assert everything.”

**Verdict:** A good *stabilizer* for prose that must remain prose, but not the primary navigation mechanism.

---

### Option E — Session capsules / structured worklogs (P1 Option 4; P2 Option 4)

**What it is:** Append-only structured logs per task/PR capturing hypotheses, decisions, pitfalls, validations, plus links to touched files/symbols; enforced for high-impact diffs.

**Strengths**

* Directly addresses the “session-only truth” problem you call out: ruled-out hypotheses and pitfalls are captured when they occur, not guessed later by doc-gardening.
* Becomes a powerful retrieval primitive when indexed by file/symbol/tag.

**Weaknesses / risks**

* Can become bureaucratic if mandatory for trivial changes; can devolve into low-signal filler unless templated and partially auto-filled.

**Verdict:** Strong complement—make it **thresholded** (API/architecture/migrations/large diffs) and tool-assisted.

---

## 5) Comparative summary (against your two explicit churns + enforcement)

| Option                    |    Relevance churn |                   IO churn |     Commit-gated freshness | Captures session-only context | Complexity risk |
| ------------------------- | -----------------: | -------------------------: | -------------------------: | ----------------------------: | --------------: |
| Generated index + KB tool |           High win |                   High win | High (regen + diff gating) |       Medium (needs capsules) |          Medium |
| Module cards / manifests  |        Medium–High | Medium (unless aggregated) |  Medium–High (path gating) |                    Low–Medium |          Medium |
| Fact graph                |           High win |                   High win |       High (anchor hashes) |                   Medium–High |            High |
| Verifiable assertions     | Medium (for prose) |                        Low |                Medium–High |                           Low |          Medium |
| Session capsules          |             Medium |                     Medium |  Medium (threshold gating) |                      High win |          Medium |

This is consistent with the “design target” language in Proposal 1 (queryable, structured, commit-gated)  and the “atomic, stable, single-call retrieval + deterministic gates” goals in Proposal 2.

---

## 6) Recommendation (based on evaluation, not inventing a third plan)

If the question is “which plan options are strongest,” the combined recommendation from both proposals is coherent and well-justified:

1. **Make a generated structural index + single KB query/pack tool the primary navigation substrate** (Proposal 1 Option 1 / Proposal 2 Option 3). This is the only option that directly and systematically attacks both churn types while enabling deterministic gating.
2. **Add thin, scoped human-authored overlays**:

   * module cards/manifests (for “why” and “edit points”) where needed, not everywhere (Proposal 1 Option 2 / Proposal 2 Option 1).
3. **Introduce session capsules/worklogs with thresholds** for decision-heavy changes (Proposal 1 Option 4 / Proposal 2 Option 4) to prevent “doc gardening can’t rediscover this” loss.
4. **Add verifiable assertions selectively** for the prose you must keep, to turn drift into a failing invariant (Proposal 1 Option 3).
5. **Defer the full fact graph** until you have stable generators, enforcement, and clear “minimal fact types” governance; otherwise it risks ontology sprawl (Proposal 2 Option 2).

This is essentially Proposal 2’s staged rollout recommendation, with Proposal 1’s obligations/diff-gating as an especially important enforcement amplifier.

---

## Concise reasoning narrative

I derived evaluation criteria directly from your background constraints (relevance churn, IO churn, and commit-gated in-session updates) , then mapped the two proposals’ options onto those criteria. Both proposals converge on “structured data + single-call retrieval + deterministic gates” as the core mechanism  , and the comparative analysis indicates the generated index + KB tool is the only option that fully and systematically addresses the navigation gap while keeping the knowledge base current under mechanical enforcement, consistent with the Harness Engineering “map, not manual” philosophy.
