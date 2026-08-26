import { useEffect, useMemo, useState } from 'react';
import { acpListProviderDetails, acpListProviderModels } from '../../acp/providers';
import type {
  ResearchModelSelection,
  ResearchTeamConfiguration,
  ResearchTeamMode,
} from '../../types/sessionExperience';
import { researchTeamSize } from '../../types/sessionExperience';

interface ResearchModelOption extends ResearchModelSelection {
  providerLabel: string;
}

interface ResearchModelTeamSelectorProps {
  currentModel: string | null;
  currentProvider: string | null;
  value: ResearchTeamConfiguration;
  onChange(value: ResearchTeamConfiguration): void;
  onValidationChange(message: string | null): void;
}

const MODES: {
  mode: ResearchTeamMode;
  label: string;
  description: string;
  detail: string;
}[] = [
  {
    mode: 'solo',
    label: 'Solo',
    description: 'One model',
    detail: 'Fastest and lowest cost',
  },
  {
    mode: 'dual',
    label: 'Dual',
    description: 'Two models',
    detail: 'Independent drafts and critique',
  },
  {
    mode: 'trio',
    label: 'Trio',
    description: 'Three models',
    detail: 'Broadest coverage and critique',
  },
];

const SEAT_LABELS = ['Lead model', 'Researcher 2', 'Researcher 3'];

const MODE_SUMMARIES: Record<ResearchTeamMode, string> = {
  solo: 'One model researches and writes the final report.',
  dual: 'Two models research independently, critique once, and the lead writes the final report.',
  trio: 'Three models research independently, critique once, and the lead resolves their findings.',
};

function modelKey(model: ResearchModelSelection): string {
  return JSON.stringify([model.provider, model.model]);
}

function sameModels(left: ResearchModelSelection[], right: ResearchModelSelection[]): boolean {
  return (
    left.length === right.length &&
    left.every((model, index) => modelKey(model) === modelKey(right[index]))
  );
}

export function fillResearchModelSeats(
  mode: ResearchTeamMode,
  selected: ResearchModelSelection[],
  options: ResearchModelOption[],
  preferred?: ResearchModelSelection
): ResearchModelSelection[] {
  const size = researchTeamSize(mode);
  const available = new Map(options.map((option) => [modelKey(option), option]));
  const filled: ResearchModelSelection[] = [];
  const candidates = [...selected, preferred, ...options].filter(
    (candidate): candidate is ResearchModelSelection => Boolean(candidate)
  );

  for (const candidate of candidates) {
    const key = modelKey(candidate);
    if (!available.has(key) || filled.some((model) => modelKey(model) === key)) continue;
    filled.push({ provider: candidate.provider, model: candidate.model });
    if (filled.length === size) break;
  }
  return filled;
}

export function ResearchModelTeamSelector({
  currentModel,
  currentProvider,
  value,
  onChange,
  onValidationChange,
}: ResearchModelTeamSelectorProps) {
  const [options, setOptions] = useState<ResearchModelOption[]>([]);
  const [loading, setLoading] = useState(true);
  const [catalogMessage, setCatalogMessage] = useState<string | null>(null);
  const preferred = useMemo(
    () =>
      currentProvider && currentModel
        ? { provider: currentProvider, model: currentModel }
        : undefined,
    [currentModel, currentProvider]
  );
  const selectableOptions = useMemo(() => {
    if (!preferred || options.some((option) => modelKey(option) === modelKey(preferred))) {
      return options;
    }
    return [
      ...options,
      {
        ...preferred,
        providerLabel: preferred.provider,
      },
    ].sort((left, right) =>
      `${left.providerLabel}/${left.model}`.localeCompare(`${right.providerLabel}/${right.model}`)
    );
  }, [options, preferred]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setCatalogMessage(null);
    void acpListProviderDetails()
      .then(async (providers) => {
        const configured = providers.filter((provider) => provider.is_configured);
        const results = await Promise.allSettled(
          configured.map(async (provider) => ({
            provider,
            models: await acpListProviderModels(provider.name),
          }))
        );
        if (cancelled) return;

        const loaded: ResearchModelOption[] = [];
        let failures = 0;
        for (const result of results) {
          if (result.status === 'rejected') {
            failures += 1;
            continue;
          }
          for (const model of result.value.models) {
            loaded.push({
              provider: result.value.provider.name,
              providerLabel: result.value.provider.metadata.display_name,
              model: model.id,
            });
          }
        }
        loaded.sort((left, right) =>
          `${left.providerLabel}/${left.model}`.localeCompare(
            `${right.providerLabel}/${right.model}`
          )
        );
        setOptions(loaded);
        if (failures > 0) {
          setCatalogMessage(
            `${failures} configured provider catalog${failures === 1 ? '' : 's'} could not be loaded.`
          );
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setCatalogMessage(
            `Could not load research models: ${error instanceof Error ? error.message : String(error)}`
          );
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (loading) return;
    const filled =
      value.mode === 'solo' && value.models.length === 0
        ? []
        : fillResearchModelSeats(value.mode, value.models, selectableOptions, preferred);
    if (!sameModels(filled, value.models)) {
      onChange({ ...value, models: filled });
    }
  }, [loading, onChange, preferred, selectableOptions, value]);

  const requiredSeats = researchTeamSize(value.mode);
  const selectionIsComplete =
    value.mode === 'solo' ||
    (value.models.length === requiredSeats &&
      new Set(value.models.map(modelKey)).size === requiredSeats);

  useEffect(() => {
    if (selectionIsComplete) {
      onValidationChange(null);
      return;
    }
    onValidationChange(
      loading
        ? 'Research models are still loading.'
        : `Choose ${requiredSeats} distinct models for ${value.mode} research.`
    );
  }, [loading, onValidationChange, requiredSeats, selectionIsComplete, value.mode]);

  const changeMode = (mode: ResearchTeamMode) => {
    onChange({
      mode,
      models: fillResearchModelSeats(mode, value.models, selectableOptions, preferred),
    });
  };

  const changeSeat = (seat: number, encoded: string) => {
    if (!encoded) {
      onChange({ ...value, models: value.models.filter((_, index) => index !== seat) });
      return;
    }
    const [provider, model] = JSON.parse(encoded) as [string, string];
    const models = [...value.models];
    models[seat] = { provider, model };
    onChange({ ...value, models });
  };

  const selectedMode = MODES.find(({ mode }) => mode === value.mode) ?? MODES[0];
  const currentModelOption =
    currentProvider && currentModel
      ? `Use current — ${currentProvider}/${currentModel}`
      : 'Use current model';

  return (
    <section
      className="mb-3 rounded-xl border border-border-primary bg-background-secondary p-4"
      aria-labelledby="research-mode-title"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-[10px] font-medium uppercase tracking-wider text-text-secondary">
            Research team
          </p>
          <h2 id="research-mode-title" className="mt-0.5 text-base font-medium text-text-primary">
            Choose research mode
          </h2>
          <p className="mt-1 text-xs text-text-secondary">
            Choose how many models should investigate this request. You can assign each model below.
          </p>
        </div>
        <span
          className="shrink-0 rounded-full border border-border-active bg-background-primary px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide text-text-primary"
          aria-live="polite"
        >
          {selectedMode.label} selected
        </span>
      </div>

      <fieldset className="mt-3">
        <legend className="sr-only">Research mode</legend>
        <div
          className="grid grid-cols-1 gap-2 sm:grid-cols-3"
          role="radiogroup"
          aria-label="Research mode"
        >
          {MODES.map(({ mode, label, description, detail }) => {
            const selected = value.mode === mode;
            const unavailable =
              mode !== 'solo' && selectableOptions.length < researchTeamSize(mode);
            return (
              <label
                key={mode}
                className={`relative flex min-h-20 items-start gap-2.5 rounded-lg border p-3 transition-colors ${
                  selected
                    ? 'border-border-active bg-background-primary text-text-primary ring-1 ring-border-active'
                    : 'border-border-primary text-text-secondary hover:border-border-active hover:bg-background-primary/50'
                } ${unavailable ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}`}
              >
                <input
                  type="radio"
                  name="research-team-mode"
                  value={mode}
                  aria-label={`${label} research mode`}
                  checked={selected}
                  disabled={unavailable}
                  onChange={() => changeMode(mode)}
                  className="sr-only"
                />
                <span
                  className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                    selected ? 'border-text-primary' : 'border-border-primary'
                  }`}
                  aria-hidden="true"
                >
                  {selected && <span className="h-2 w-2 rounded-full bg-text-primary" />}
                </span>
                <span className="min-w-0">
                  <span className="block text-sm font-semibold text-text-primary">{label}</span>
                  <span className="mt-0.5 block text-xs font-medium">{description}</span>
                  <span className="mt-1 block text-[10px] leading-4">{detail}</span>
                </span>
              </label>
            );
          })}
        </div>
      </fieldset>

      <p
        className="mt-3 rounded-md border border-border-primary bg-background-primary/70 px-3 py-2 text-xs text-text-primary"
        role="status"
      >
        <span className="font-semibold">{selectedMode.label}:</span> {MODE_SUMMARIES[value.mode]}
      </p>

      <div className="mt-3 border-t border-border-primary pt-3">
        <div className="flex items-baseline justify-between gap-3">
          <h3 className="text-sm font-medium text-text-primary">Assign models</h3>
          <span className="text-[10px] text-text-secondary">
            {requiredSeats} {requiredSeats === 1 ? 'seat' : 'seats'}
          </span>
        </div>
        <div className="mt-2 grid gap-2 sm:grid-cols-3">
          {Array.from({ length: requiredSeats }, (_, seat) => {
            const selectedKey = value.models[seat] ? modelKey(value.models[seat]) : '';
            const selectedElsewhere = new Set(
              value.models.filter((_, index) => index !== seat).map(modelKey)
            );
            return (
              <label key={seat} className="min-w-0 text-xs font-medium text-text-primary">
                {SEAT_LABELS[seat]}
                <select
                  aria-label={SEAT_LABELS[seat]}
                  value={selectedKey}
                  disabled={loading || selectableOptions.length === 0}
                  onChange={(event) => changeSeat(seat, event.target.value)}
                  className="mt-1 w-full rounded-md border border-border-primary bg-background-primary px-2 py-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <option value="">
                    {seat === 0 && value.mode === 'solo' ? currentModelOption : 'Choose model…'}
                  </option>
                  {selectableOptions.map((option) => {
                    const key = modelKey(option);
                    return (
                      <option key={key} value={key} disabled={selectedElsewhere.has(key)}>
                        {option.providerLabel} — {option.model}
                      </option>
                    );
                  })}
                </select>
              </label>
            );
          })}
        </div>
      </div>

      {loading && <p className="mt-2 text-xs text-text-secondary">Loading configured models…</p>}
      {catalogMessage && (
        <p className="mt-2 text-xs text-warning" role="status">
          {catalogMessage}
        </p>
      )}
      {!loading && selectableOptions.length < 2 && (
        <p className="mt-2 text-xs text-text-secondary">
          Configure at least two models to enable Dual research, or continue in Solo mode.
        </p>
      )}
    </section>
  );
}
