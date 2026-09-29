import { useEffect, useState, useCallback } from 'react';
import { all_gosling_modes, ModeSelectionItem } from './ModeSelectionItem';
import { useConfig } from '../../ConfigContext';
import { ConversationLimitsDropdown } from './ConversationLimitsDropdown';
import { CodeExecutionRuntimeSection } from './CodeExecutionRuntimeSection';

/// Must match `#[default]` on `GoslingMode` in
/// `crates/gosling-providers/src/gosling_mode.rs`. Showing `auto` here while
/// the backend defaulted to another mode told the operator that every tool
/// call was auto-approved when it was not. (WFG-GOS-001)
const DEFAULT_GOSLING_MODE = 'auto';

/// Must match `DEFAULT_MAX_TURNS` in `crates/gosling/src/agents/agent.rs`, which
/// the backend uses when `GOSLING_MAX_TURNS` does not parse as a `u32`.
const DEFAULT_MAX_TURNS = 1000;
const MAX_U32 = 4294967295;

const parseMaxTurns = (value: unknown): number | null => {
  const digits = typeof value === 'number' ? String(value) : String(value).trim();
  if (!/^\d+$/.test(digits)) {
    return null;
  }
  const turns = Number(digits);
  return turns <= MAX_U32 ? turns : null;
};

export const ModeSection = () => {
  const [currentMode, setCurrentMode] = useState(DEFAULT_GOSLING_MODE);
  const [maxTurns, setMaxTurns] = useState<number>(DEFAULT_MAX_TURNS);
  const [invalidMaxTurns, setInvalidMaxTurns] = useState<string | null>(null);
  const { config, read, upsert } = useConfig();

  const handleModeChange = async (newMode: string) => {
    try {
      await upsert('GOSLING_MODE', newMode, false);
      setCurrentMode(newMode);
    } catch (error) {
      console.error('Error updating gosling mode:', error);
      throw new Error(`Failed to store new gosling mode: ${newMode}`);
    }
  };

  useEffect(() => {
    const mode = config.GOSLING_MODE as string | undefined;
    if (mode) {
      setCurrentMode(mode);
    }
  }, [config.GOSLING_MODE]);

  const fetchStoredSettings = useCallback(async () => {
    try {
      // The shared config snapshot is loaded once per renderer, so re-read the file to
      // pick up edits made outside the app since then.
      const [mode, turns] = await Promise.all([
        read('GOSLING_MODE', false),
        read('GOSLING_MAX_TURNS', false),
      ]);
      if (typeof mode === 'string' && mode) {
        setCurrentMode(mode);
      }
      if (turns == null) {
        return;
      }
      const parsedTurns = parseMaxTurns(turns);
      if (parsedTurns === null) {
        setInvalidMaxTurns(typeof turns === 'object' ? JSON.stringify(turns) : String(turns));
      } else {
        setMaxTurns(parsedTurns);
        setInvalidMaxTurns(null);
      }
    } catch (error) {
      console.error('Error reading mode and max turns:', error);
    }
  }, [read]);

  const handleMaxTurnsChange = async (value: number) => {
    try {
      await upsert('GOSLING_MAX_TURNS', value, false);
      setMaxTurns(value);
      setInvalidMaxTurns(null);
    } catch (error) {
      console.error('Error updating max turns:', error);
    }
  };

  useEffect(() => {
    fetchStoredSettings();
  }, [fetchStoredSettings]);

  return (
    <div className="space-y-1">
      {/* Mode Selection */}
      {all_gosling_modes.map((mode) => (
        <ModeSelectionItem
          key={mode.key}
          mode={mode}
          currentMode={currentMode}
          showDescription={true}
          isApproveModeConfigure={false}
          handleModeChange={handleModeChange}
        />
      ))}

      {/* Conversation Limits Dropdown */}
      <ConversationLimitsDropdown
        maxTurns={maxTurns}
        invalidMaxTurns={invalidMaxTurns}
        defaultMaxTurns={DEFAULT_MAX_TURNS}
        onMaxTurnsChange={handleMaxTurnsChange}
      />

      <CodeExecutionRuntimeSection />
    </div>
  );
};
