import { describe, expect, it } from 'vitest';
import { zDiagnosticsReportLevel, zProviderTypeDto, zRole } from '@repo-makeover/gosling-sdk';
import type { DiagnosticsLevel } from './diagnostics';
import type { Role } from './message';
import type { ProviderType } from './providers';

/**
 * Guards hand-maintained TypeScript string-union "enums" in `types/*.ts`
 * against drift from the Rust enums they mirror.
 *
 * Nothing forces a hand-written union like `ProviderType` to stay in sync
 * if the Rust enum it mirrors (in `crates/gosling-sdk-types`) gains, loses,
 * or renames a variant. `@repo-makeover/gosling-sdk`'s `zod.gen.ts` *is*
 * generated straight from the Rust ACP schema (`crates/gosling/acp-schema.json`,
 * kept current via `just generate-acp-types` and enforced by the
 * `check-acp-schema` CI gate), so it's the canonical source of truth to
 * diff hand mirrors against.
 *
 * Each `satisfies Record<X, true>` literal below is checked by `tsc` for
 * exact key correspondence with the hand-written type `X` (missing or
 * extra keys are compile errors), so the array it produces is a verified
 * projection of the type -- not a second, independently-maintained copy
 * that could itself drift. The runtime assertion then diffs that array
 * against the generated zod schema's literal values.
 */
function schemaLiteralValues(schema: { options: readonly unknown[] }): string[] {
  return schema.options
    .map((option) => (typeof option === 'string' ? option : (option as { value: string }).value))
    .sort();
}

describe('hand-maintained TS enum mirrors stay in sync with generated Rust-derived types', () => {
  it('ProviderType (types/providers.ts) matches ProviderTypeDto', () => {
    const handMirror = Object.keys({
      Preferred: true,
      Builtin: true,
      Declarative: true,
      Custom: true,
    } satisfies Record<ProviderType, true>).sort();

    expect(handMirror).toEqual(schemaLiteralValues(zProviderTypeDto));
  });

  it('DiagnosticsLevel (types/diagnostics.ts) matches DiagnosticsReportLevel', () => {
    const handMirror = Object.keys({
      summary: true,
      full: true,
    } satisfies Record<DiagnosticsLevel, true>).sort();

    expect(handMirror).toEqual(schemaLiteralValues(zDiagnosticsReportLevel));
  });

  it('Role (types/message.ts) matches Role', () => {
    const handMirror = Object.keys({
      user: true,
      assistant: true,
    } satisfies Record<Role, true>).sort();

    expect(handMirror).toEqual(schemaLiteralValues(zRole));
  });
});
