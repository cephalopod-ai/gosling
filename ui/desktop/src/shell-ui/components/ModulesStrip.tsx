import type { ShellModuleSummary } from '@repo-makeover/gosling-sdk';
import type { ShellRuntimeSnapshot } from '../../shell/runtimeSnapshot';
import { COPY } from '../copy';
import { ShellNotice, type ShellTone } from './primitives';

const MODULE_TONES: Record<ShellModuleSummary['status'], ShellTone> = {
  ready: 'ok',
  unavailable: 'warn',
  incompatible: 'error',
};

const ADAPTER_TONES: Record<NonNullable<ShellRuntimeSnapshot['adapter']>['status'], ShellTone> = {
  ready: 'ok',
  crashed: 'error',
  hung: 'warn',
  incompatible: 'error',
};

export interface ModulesStripProps {
  modules: ShellModuleSummary[];
  adapter: ShellRuntimeSnapshot['adapter'];
  domainDeclared: boolean;
  onSaveDiagnostics: () => void;
}

/**
 * Hidden when the inventory is only `core:session` and no adapter exists. A provisioned module the
 * backend could not resolve stays listed as `unavailable` so recovery remains visible.
 */
export const ModulesStrip = ({
  modules,
  adapter,
  domainDeclared,
  onSaveDiagnostics,
}: ModulesStripProps) => {
  const interesting = modules.filter((module) => module.kind !== 'core');
  if (interesting.length === 0 && !adapter) return null;

  const degradedModule = modules.some((module) => module.status !== 'ready');
  const degradedAdapter = adapter !== null && adapter.status !== 'ready';

  return (
    <section className="gsh-modules" aria-label="Declared modules">
      {degradedModule ? (
        <ShellNotice tone="warn" message={COPY.moduleUnavailable} live>
          <button type="button" className="gsh-btn gsh-btn--ghost" onClick={onSaveDiagnostics}>
            {COPY.saveDiagnostics}
          </button>
        </ShellNotice>
      ) : null}
      {degradedAdapter ? (
        <ShellNotice tone="error" message={COPY.adapterUnavailable} live>
          <button type="button" className="gsh-btn gsh-btn--ghost" onClick={onSaveDiagnostics}>
            {COPY.saveDiagnostics}
          </button>
        </ShellNotice>
      ) : null}
      <ul className="gsh-modules__list">
        {modules.map((module) => (
          <li
            key={`${module.kind}:${module.id}`}
            className={`gsh-module gsh-module--${MODULE_TONES[module.status]}`}
          >
            <span className="gsh-module__id">{module.id}</span>
            <span className="gsh-module__kind">{module.kind}</span>
            <span className="gsh-module__status">{module.status}</span>
            {module.version ? <span className="gsh-module__version">{module.version}</span> : null}
          </li>
        ))}
        {adapter ? (
          <li className={`gsh-module gsh-module--${ADAPTER_TONES[adapter.status]}`}>
            <span className="gsh-module__id">{adapter.descriptorId}</span>
            <span className="gsh-module__kind">adapter</span>
            <span className="gsh-module__status">{adapter.status}</span>
            <span className="gsh-module__version">{adapter.protocolVersion}</span>
            {domainDeclared && adapter.actions.length > 0 ? (
              <span className="gsh-module__actions">{adapter.actions.join(', ')}</span>
            ) : null}
          </li>
        ) : null}
      </ul>
    </section>
  );
};
