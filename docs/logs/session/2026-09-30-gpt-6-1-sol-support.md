# 2026-09-30 — GPT-6.1 Sol support

## Task

Add `gpt-6.1-sol` to Gosling's OpenAI and ChatGPT Codex model inventories without changing provider defaults.

## Implementation

- Added the model to direct OpenAI fallback metadata and the pending canonical catalog with OpenAI's published limits, modalities, knowledge cutoff, release date, and token pricing.
- Added ChatGPT Codex fallback reasoning levels and effective context limit. Existing GPT-6 Responses routing and reasoning normalization cover the new identifier.
- Extended request, model, and catalog regression tests and updated the OpenAI provider guide.

## Validation

- `RUSTUP_AUTO_INSTALL=0 cargo fmt --all`: passed.
- `RUSTUP_AUTO_INSTALL=0 cargo test -p gosling-providers --lib --locked`: 491 passed.
- `RUSTUP_AUTO_INSTALL=0 cargo test -p gosling --lib providers::chatgpt_codex::tests --locked`: 44 passed.
- `git diff --check`: passed after the patch.

## Risks and follow-ups

- No live request was made. API and ChatGPT Codex access depends on the configured account and route availability; deterministic tests cover request construction and catalog behavior.
