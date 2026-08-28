# Rename manifest

Date: 2026-08-27

No files, modules, public types, or public API symbols were renamed.

| Private test symbol | Replacement | Reference proof |
|---|---|---|
| `test_basic_config` | `config_values_support_environment_overrides` | Definition-only Rust unit test; exact-name scan after the change. |
| `test_basic_response` | `assert_basic_text_response` | Private helper and its single call site changed together; exact-name scan after the change. |

The PromptManager snapshot test retains its existing `test_basic` name because
`insta` uses the function name as the committed snapshot identity. The initial
rename produced a new snapshot key during validation and was reverted rather
than creating a content-identical snapshot rename.
