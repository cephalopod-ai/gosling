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

const MODES: { mode: ResearchTeamMode; label: string; description: string }[] = [
  { mode: 'solo', label: 'Solo', description: 'One model' },
  { mode: 'dual', label: 'Dual', description: 'Two models' },
  { mode: 'trio', label: 'Trio', description: 'Three models' },
];

const SEAT_LABELS = ['Lead model', 'Researcher 2', 'Researcher 3'];

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
  const candidates = [preferred, ...selected, ...options].filter(
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
        : fillResearchModelSeats(value.mode, value.models, options, preferred);
    if (!sameModels(filled, value.models)) {
      onChange({ ...value, models: filled });
    }
  }, [loading, onChange, options, preferred, value]);

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
      models: fillResearchModelSeats(mode, value.models, options, preferred),
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

  return (
    <fieldset className="mt-3 border-t border-border-primary pt-3">
      <legend className="text-sm font-medium text-text-primary">Research models</legend>
      <p className="mt-1 text-xs text-text-secondary">
        Dual and Trio run independent drafts in parallel, one critique round, then lead synthesis.
      </p>
      <div
        className="mt-2 grid grid-cols-3 gap-2"
        role="radiogroup"
        aria-label="Research team size"
      >
        {MODES.map(({ mode, label, description }) => {
          const unavailable = mode !== 'solo' && options.length < researchTeamSize(mode);
          return (
            <label
              key={mode}
              className={`rounded-md border px-2 py-2 text-center text-xs ${
                value.mode === mode
                  ? 'border-text-primary bg-background-primary text-text-primary'
                  : 'border-border-primary text-text-secondary'
              } ${unavailable ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}`}
            >
              <input
                type="radio"
                name="research-team-mode"
                value={mode}
                checked={value.mode === mode}
                disabled={unavailable}
                onChange={() => changeMode(mode)}
                className="sr-only"
              />
              <span className="block font-medium">{label}</span>
              <span className="block text-[10px]">{description}</span>
            </label>
          );
        })}
      </div>

      <div className="mt-3 grid gap-2 sm:grid-cols-3">
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
                disabled={loading || options.length === 0}
                onChange={(event) => changeSeat(seat, event.target.value)}
                className="mt-1 w-full rounded-md border border-border-primary bg-background-primary px-2 py-1.5 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="">
                  {seat === 0 && value.mode === 'solo' ? 'Use current model' : 'Choose model…'}
                </option>
                {options.map((option) => {
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

      {loading && <p className="mt-2 text-xs text-text-secondary">Loading configured models…</p>}
      {catalogMessage && (
        <p className="mt-2 text-xs text-warning" role="status">
          {catalogMessage}
        </p>
      )}
      {!loading && options.length < 2 && (
        <p className="mt-2 text-xs text-text-secondary">
          Configure at least two models to enable Dual research, or continue in Solo mode.
        </p>
      )}
    </fieldset>
  );
}
