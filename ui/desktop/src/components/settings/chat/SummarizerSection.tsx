import { useCallback, useEffect, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import { acpListSummarizerModels } from '../../../acp/providers';
import { useConfig } from '../../ConfigContext';
import { Button } from '../../ui/button';
import { Input } from '../../ui/input';
import { Select } from '../../ui/Select';

interface ModelOption {
  value: string;
  label: string;
}

interface SummarizerModeOption {
  key: string;
  label: string;
  description: string;
}

// Mirrors the three modes of the Slice 1 GOSLING_CONTEXT_MANAGER selector so
// the two read consistently, with the inline explanations shown to the user.
const SUMMARIZER_MODES: SummarizerModeOption[] = [
  {
    key: 'off',
    label: 'Off',
    description: 'Disabled; no summarizer runs, no memory writes (default, zero cost).',
  },
  {
    key: 'shadow',
    label: 'Shadow',
    description:
      'Observe only; logs the digest and would-be memory writes without changing what the model sees or writing to disk.',
  },
  {
    key: 'on',
    label: 'On',
    description:
      'Active; real summaries replace truncation and facts are written to memories.jsonl for durable recall.',
  },
];

const DEFAULT_ENDPOINT_PLACEHOLDER = 'http://localhost:11434/v1';
const DEFAULT_MODEL_PLACEHOLDER = 'qwen2.5-coder:3b';
const DEFAULT_TIMEOUT_MS = 4000;

export const SummarizerSection = () => {
  const { read, upsert } = useConfig();

  const [mode, setMode] = useState('off');
  const [endpoint, setEndpoint] = useState('');
  const [model, setModel] = useState('');
  const [timeoutMs, setTimeoutMs] = useState<string>(String(DEFAULT_TIMEOUT_MS));

  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  const [visibleModelOptions, setVisibleModelOptions] = useState<ModelOption[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [modelFetchError, setModelFetchError] = useState<string | null>(null);
  const [lastFetchedEndpoint, setLastFetchedEndpoint] = useState<string | null>(null);

  const fetchModels = useCallback(async (endpointToFetch: string) => {
    const trimmed = endpointToFetch.trim();
    if (!trimmed) return;
    setLoadingModels(true);
    setModelFetchError(null);
    try {
      const models = await acpListSummarizerModels(trimmed);
      const options = models.map((id) => ({ value: id, label: id }));
      setModelOptions(options);
      setVisibleModelOptions(options);
      setLastFetchedEndpoint(trimmed);
    } catch (error) {
      console.error('Error listing summarizer models:', error);
      setModelOptions([]);
      setVisibleModelOptions([]);
      setModelFetchError('Could not list models from this endpoint.');
    } finally {
      setLoadingModels(false);
    }
  }, []);

  const loadSettings = useCallback(async () => {
    try {
      const storedMode = (await read('GOSLING_SUMMARIZER', false)) as string | undefined;
      if (storedMode) setMode(storedMode);

      const storedEndpoint = (await read('GOSLING_SUMMARIZER_ENDPOINT', false)) as
        | string
        | undefined;
      // Pre-fill with a real, working default rather than leaving the field
      // empty behind placeholder text — first-time users get a value that
      // both the summarizer worker and model detection can actually use.
      const effectiveEndpoint = storedEndpoint || DEFAULT_ENDPOINT_PLACEHOLDER;
      setEndpoint(effectiveEndpoint);
      fetchModels(effectiveEndpoint);
      if (!storedEndpoint) {
        upsert('GOSLING_SUMMARIZER_ENDPOINT', effectiveEndpoint, false).catch((error) => {
          console.error('Error saving default summarizer endpoint:', error);
        });
      }

      const storedModel = (await read('GOSLING_SUMMARIZER_MODEL', false)) as string | undefined;
      if (storedModel) setModel(storedModel);

      const storedTimeout = (await read('GOSLING_SUMMARIZER_TIMEOUT_MS', false)) as
        | number
        | undefined;
      if (storedTimeout) setTimeoutMs(String(storedTimeout));
    } catch (error) {
      console.error('Error loading summarizer settings:', error);
    }
  }, [read, upsert, fetchModels]);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleModeChange = async (newMode: string) => {
    setMode(newMode);
    try {
      await upsert('GOSLING_SUMMARIZER', newMode, false);
    } catch (error) {
      console.error('Error updating summarizer mode:', error);
    }
  };

  const persistEndpoint = async () => {
    const trimmed = endpoint.trim();
    try {
      await upsert('GOSLING_SUMMARIZER_ENDPOINT', trimmed, false);
    } catch (error) {
      console.error('Error updating summarizer endpoint:', error);
    }
    if (trimmed && trimmed !== lastFetchedEndpoint) {
      fetchModels(trimmed);
    }
  };

  const handleModelSelect = (newValue: unknown) => {
    const option = newValue as ModelOption | null;
    const nextModel = option?.value ?? '';
    setModel(nextModel);
    upsert('GOSLING_SUMMARIZER_MODEL', nextModel.trim(), false).catch((error) => {
      console.error('Error updating summarizer model:', error);
    });
  };

  const handleModelInputChange = (inputValue: string) => {
    const trimmed = inputValue.trim();
    if (!trimmed) {
      setVisibleModelOptions(modelOptions);
      return;
    }
    const matches = modelOptions.filter((option) =>
      option.value.toLowerCase().includes(trimmed.toLowerCase())
    );
    setVisibleModelOptions(
      matches.length > 0 ? matches : [{ value: trimmed, label: `Use: "${trimmed}"` }]
    );
  };

  const persistModel = async () => {
    try {
      await upsert('GOSLING_SUMMARIZER_MODEL', model.trim(), false);
    } catch (error) {
      console.error('Error updating summarizer model:', error);
    }
  };

  const persistTimeout = async () => {
    const parsed = Number(timeoutMs);
    if (!Number.isFinite(parsed) || parsed < 1) {
      setTimeoutMs(String(DEFAULT_TIMEOUT_MS));
      return;
    }
    try {
      await upsert('GOSLING_SUMMARIZER_TIMEOUT_MS', parsed, false);
    } catch (error) {
      console.error('Error updating summarizer timeout:', error);
    }
  };

  const showEndpointFields = mode !== 'off';

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        {SUMMARIZER_MODES.map((option) => {
          const checked = mode === option.key;
          return (
            <div key={option.key} className="group hover:cursor-pointer text-sm">
              <div
                className={`flex items-center justify-between text-text-primary py-2 px-2 ${
                  checked
                    ? 'bg-background-secondary'
                    : 'bg-background-primary hover:bg-background-secondary'
                } rounded-lg transition-all`}
                onClick={() => handleModeChange(option.key)}
              >
                <div className="flex">
                  <div>
                    <h3 className="text-text-primary">{option.label}</h3>
                    <p className="text-text-secondary mt-[2px]">{option.description}</p>
                  </div>
                </div>
                <div className="relative flex items-center gap-2">
                  <input
                    type="radio"
                    name="summarizer-mode"
                    value={option.key}
                    checked={checked}
                    onChange={() => handleModeChange(option.key)}
                    className="peer sr-only"
                  />
                  <div
                    className="h-4 w-4 rounded-full border border-border-primary
                      peer-checked:border-[6px] peer-checked:border-black dark:peer-checked:border-white
                      peer-checked:bg-white dark:peer-checked:bg-black
                      transition-all duration-200 ease-in-out group-hover:border-border-primary"
                  ></div>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {showEndpointFields && (
        <div className="space-y-3 px-2">
          <div className="flex flex-col">
            <label className="text-sm font-medium mb-1 block text-text-primary">
              Endpoint (local OpenAI-compatible URL)
            </label>
            <Input
              value={endpoint}
              placeholder={DEFAULT_ENDPOINT_PLACEHOLDER}
              onChange={(e) => setEndpoint(e.target.value)}
              onBlur={persistEndpoint}
              className="text-text-primary"
            />
          </div>

          <div className="flex flex-col">
            <div className="flex items-center justify-between mb-1">
              <label className="text-sm font-medium text-text-primary">Model</label>
              <Button
                type="button"
                variant="ghost"
                size="xs"
                disabled={!endpoint.trim() || loadingModels}
                onClick={() => fetchModels(endpoint)}
                className="text-text-secondary"
              >
                <RefreshCw className={`h-3 w-3 ${loadingModels ? 'animate-spin' : ''}`} />
                {loadingModels ? 'Detecting…' : 'Detect models'}
              </Button>
            </div>
            {modelOptions.length > 0 ? (
              <Select
                options={visibleModelOptions}
                value={model ? { value: model, label: model } : null}
                onChange={handleModelSelect}
                onInputChange={handleModelInputChange}
                placeholder={DEFAULT_MODEL_PLACEHOLDER}
                isClearable
              />
            ) : (
              <Input
                value={model}
                placeholder={DEFAULT_MODEL_PLACEHOLDER}
                onChange={(e) => setModel(e.target.value)}
                onBlur={persistModel}
                className="text-text-primary"
              />
            )}
            {modelFetchError && (
              <p className="text-xs text-text-secondary mt-1">{modelFetchError}</p>
            )}
          </div>

          <div className="flex flex-col">
            <label className="text-sm font-medium mb-1 block text-text-primary">Timeout (ms)</label>
            <Input
              type="number"
              min={1}
              value={timeoutMs}
              placeholder={String(DEFAULT_TIMEOUT_MS)}
              onChange={(e) => setTimeoutMs(e.target.value)}
              onBlur={persistTimeout}
              className="text-text-primary"
            />
          </div>
        </div>
      )}
    </div>
  );
};
