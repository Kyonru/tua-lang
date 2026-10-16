---
icon: lucide/bot
---

# LLM Language Evaluation

Tua includes a deterministic, provider-neutral benchmark for measuring whether
a language model can produce valid Tua rather than merely plausible-looking
Lua, Luau, or TypeScript. It contains 40 fill-in-the-middle tasks covering Lua
preservation, types, return paths, modules, classes, LOVE APIs, compound
assignment, deep field access, and common foreign-syntax traps.

The benchmark is a language-learning check, not a code-similarity test. A
candidate must:

1. satisfy narrow task requirements and reject known contaminating syntax;
2. pass a fresh whole-project `tua check`;
3. pass `tua build`, including lowering to generated Lua.

Reference answers are used only to prove that every task is satisfiable. They
are never included in exported model requests. Each task also has a negative
control that must fail, preventing an accidentally permissive oracle from
turning the suite green.

## Offline Corpus Verification

Run the deterministic gate used by local verification and CI:

```bash
make llm-language-eval-test
```

Reports are written to `target/llm-language/reference-report.json` and
`target/llm-language/mutant-report.json`. This command does not contact a model
or execute generated Lua. Every candidate is checked in a fresh temporary
project, compiler processes have a timeout, and temporary projects are removed
after scoring.

## Evaluate Any Model

Export newline-delimited requests without answers:

```bash
node scripts/llm-language-eval.js \
  --emit-requests target/llm-language/requests.jsonl
```

Each line contains `id`, `category`, `instruction`, `prompt`, `suffix`, and the
required raw-source response format. Supply model results as JSONL with one
record per task:

```json
{"id":"lua-local-type","candidate":"local score: number = 10\n","model":"example"}
```

Then score them with the same compiler-backed oracle:

```bash
node scripts/llm-language-eval.js \
  --responses responses.jsonl \
  --out target/llm-language/model-report.json
```

Missing responses fail explicitly. The report includes strict totals,
per-category totals, the rejection stage, a bounded compiler message, optional
model identity, and optional latency supplied by the response file. Exact text
matching, BLEU, and reference-answer similarity are not used.

## Compact Prompt Layers

The benchmark compares models with a compact 300–500-word core contract plus
one task card when the task needs types, functions, modules, classes, or LOVE.
The cards live beside the corpus under `benchmarks/llm-language/cards/`; the
complete public language contract remains [`llms.txt`](llms.txt). This keeps
routine prompts small while the full contract remains available for retrieval
and unfamiliar tasks.

## Optional Ollama Run

An installed local Ollama model can be evaluated end to end:

```bash
TUA_OLLAMA_MODEL=qwen2.5-coder:7b make llm-language-eval-ollama
```

`TUA_OLLAMA_ENDPOINT` defaults to `http://127.0.0.1:11434` and is restricted to
loopback hosts. Live model evaluation is intentionally not a required CI or
release gate: model availability and stochastic output must not decide whether
the compiler is releasable. The offline references, corpus integrity, and
must-fail controls are the trusted gate.

## Interpreting Results

Use the strict pass rate as the primary result, then inspect category scores.
A high parse rate with weak module or class scores means the model has learned
surface syntax but not project semantics. Failures at the structure stage
usually indicate Markdown, foreign syntax, an invented API, or a missed task
requirement. Failures at `check` or `build` indicate language or lowering
errors.

For reproducible model comparisons, record the model identifier, quantization,
runtime version, prompt condition, temperature, and number of repetitions.
Compare task-only, core-contract, and core-plus-card conditions separately;
do not combine them into one headline score.
