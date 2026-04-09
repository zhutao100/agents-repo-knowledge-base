# Project background and mission

## Background

### Tackle the memory-less nature of transformer based LLM models for working in large and complex code projects

A fundamental difference of leveraging transformer based LLM models to work in a real-world project, v.s. a human engineer working in the project, is that

1. the model is a fixed-weight file, it does not change during inference; i.e. the model will not accumulate any "memory" across inference sessions.
2. the model inference works with a fixed context window (e.g. GPT-5.x supports a 400k token window (272k usable for input, 128k reserved for output), any project/task related context must be loaded into the context window every time an inference session starts.

This is not a problem at all for tiny size projects, i.e. for a project with 500 total LoC, we can just load the full code into the inference sessions.

For medium to big projects, this becomes more and more a major problem and hard limit; once the project size passes a threshold, to avoid context window exhaustion, an alternative mechanism must be established to render on-demand compact and compressed context to the model sessions, instead of allowing the models to fully read all the relevant code files.

### Harness engineering

The article "Harness engineering: leveraging Codex in an agent-first world".

Article access
- `docs/context/Harness engineering leveraging Codex in an agent-first world.md`
- URL: `https://openai.com/index/harness-engineering`

The article describes a recent practice of leveraging agentic tools in complex project developments.

Especially, the section "We made repository knowledge the system of record" in the article describes a mechanism/pattern to maintain and manage repo knowledge/context for LLM agents.

The core idea is maintaining the repository’s knowledge base in a structured `docs/` directory, with `AGENTS.md` as an index.

### The dilemma of the current project knowledge practice

The current project knowledge practice still suffers from the gaps and issues below.

#### The gap v.s. a human engineer working in the project

For a human engineer that is familiar with the project codebase, the pattern of starting on a feature work or bug fix is roughly,

1. understand the background and requirements.
2. browse over the codebase, identify the relevant code files, make a mental or written draft of change plan.
3. code changes, tests, verifications, etc.

For LLM agents,
- the "1." equivalent can be a well-written initial prompt in the form of PRD.
- the "3." can be streamlined and executed very well by the current models.
- the "2." is the biggest gap.

For the step "2." above, human engineers usually don't need to freshly dive into a large set of free-style markdown docs; instead, they
a) keep a high-level mental model of the project mission, architecture, layout shape.
b) start from the top-level project layout, quickly recall "the directory a is about xx", "open directory b, then open directory ba"
c) within a directory "ba", they quickly recall "file ba1 is about xxx", "file ba2 is about yyy", "open file ba3 to inspect"

For LLM models,
- the "a)" equivalent can be few well-maintained high-level docs, like the `docs/ARCHITECTURE.md` `docs/DESIGN.md` in the "harness engineering" article example; this step is usually not too big a issue for LLM agents.
- the biggest gaps the in "step 2." are usually the sub-steps "b) and "c)".

Unlike human engineers who immediately/"cheaply" recall and navigate in the project without exhausting too much energy, the dilemma of LLM agents are that,

- a context-window-friendly `AGENTS.md` and `docs/` "knowledge base" usually don't enumerate what each directory and each file is about; so even the agent has viewed some key markdown docs, it still cannot efficiently navigate and identify real relevant work place, what happens in the real world is usually either,
   - the agent find some start points of certain files or certain keywords from the "knowledge base" docs, then start text searching within the project, open all the potential match files to inspect; this results in a big context-window consumption.
   - the agent tries to be context-preserving and efficient, only pick few "most likely" matches, and only render a narrow range of lines from the match files; this can result in a incomplete or inaccurate context research.

#### The awkwardness for LLM agents to work in-session with free-text markdown

Markdown docs are free-text with optional and flexible syntax, designed for rendering to humans. There is no reliable and deterministic way to selectively retrieve/update certain information from a markdown file.

With the "harness engineering" article's approach of structured `docs/` directory with `AGENTS.md` as the agents "knowledge base",

- At context research stage,
  - the agents have no cost-efficient way to tell whether a markdown doc contains the information it needs, before reading into it.
  - the agents have to read a full markdown doc to make sure it does not miss certain information stored in it.

- When the agents finish the code changes for a task,
  - the agents only have session context of the docs (or the ranged lines of the docs) that it has read, but there might be other docs that need to be updated as well.
  - the agents have no cost-efficient way to reliably and deterministically tell which exact docs need to be updated.
  - even given a set of docs to update, the work of updating a set of free-text markdown docs itself can be very context-window-costing.

The awkwardness above combined, often inevitably results in the staleness and drift of the markdown "knowledge base" over time/tasks.

#### The awkwardness for LLM agents to update/fix the markdown knowledge base off-session

"Off-session" means out of the regular task sessions; the article describes a mechanism attempting to tackle the markdown drift over time/task as below.

```
A recurring “doc-gardening” agent scans for stale or obsolete documentation that does not reflect the real code behavior and opens fix-up pull requests.
```

However, this off-session update/fix mechanism often suffers from the failure modes as
- the best work context of updating the relevant knowledge base lives in the task sessions, and there are even a lot of implicit context that is unlikely to be re-discovered, e.g. "the ruled out hypothesis" "the pitfalls avoided or hit".
- a dedicated scan session also has to work with the LLM context window limit, so it cannot "just read all the code and docs" to have a real global view to update the docs.
- similar challenges as "work in-session with free-text markdown"; markdown docs are non-structured free-text, agents have to fully read a markdown doc to reliably update it, ranged lines of reading can result in partial context and blind edits.

## Mission

Understand the current issues of the "harness engineering" practice of setting up and maintaining "project knowledge base" for LLM agents across inference sessions.

Then, reason and propose better solution options.

### Solution requirement

The solution should set up a "project knowledge base" for LLM agents to recall the necessary context between inference sessions. The "knowledge base" system should

- optimize both relevance and IO churns; the context churn can come from both of the failure modes below
  - **relevance churn**: e.g., having to dump a whole free-text markdown doc to search for certain information
  - **io churn**: the LLM agents have to explicitly output a complete tool call block for each tool call operation; for example, a sub-directory contains 10 small (<100 LoC) code files ; if the agents have to make 10 separate tool calls to retrieve the directory information, the tool calls themselves are a big churn and overhead.
- enforce to be kept up-to-date upon each change commit.
  - have a programable and deterministic way (e.g. git pre-commit hook) to check and enforce the relevant knowledge base is updated together with the code changes to be committed;
  - given the off-session failure modes, this check must enforce in-session knowledge updates.
