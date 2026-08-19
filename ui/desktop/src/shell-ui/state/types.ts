import type { ShellSettingsSnapshot } from '../../shell/ipc';
import type { ShellInteraction } from '../../shell/interactionController';
import type { ShellOperationFailure } from '../../shell/operationFailure';
import type { ShellRuntimeSnapshot } from '../../shell/runtimeSnapshot';
import type { ShellSessionSummary } from '../../shell/acpRuntime';
import type { ShellSessionUpdate, ShellTranscriptSnapshot } from '../../shell/sessionController';
import type { ShellHandoffEnvelope } from '@repo-makeover/gosling-sdk';
import type { ShellArtifactSummary } from '@repo-makeover/gosling-sdk';
import type { TranscriptState } from './transcript';

export type ShellUiView = 'workspace' | 'sessions' | 'settings' | 'handoff';

export type ShellUiPendingOperation =
  | 'directory.select'
  | 'credential.select'
  | 'session.create'
  | 'session.list'
  | 'session.resume'
  | 'session.transcript.read'
  | 'session.artifacts.read'
  | 'session.detach'
  | 'prompt.submit'
  | 'prompt.cancel'
  | 'interaction.respond'
  | 'diagnostics.save'
  | 'handoff.prepare'
  | 'handoff.confirm'
  | 'runtime.retry'
  | 'runtime.stop'
  | 'settings.update'
  | 'settings.reset';

export interface ShellUiState {
  snapshot: ShellRuntimeSnapshot | null;
  settings: ShellSettingsSnapshot | null;
  transcript: TranscriptState;
  interactions: ShellInteraction[];
  sessions: { status: 'idle' | 'loading' | 'loaded'; items: ShellSessionSummary[] };
  outputs: {
    status: 'idle' | 'loading' | 'loaded';
    items: ShellArtifactSummary[];
    totalCount: number;
    truncated: boolean;
  };
  draft: string;
  failure: ShellOperationFailure | null;
  pending: ShellUiPendingOperation | null;
  respondedActionIds: string[];
  view: ShellUiView;
  handoff: ShellHandoffEnvelope | null;
  savedDiagnosticsFile: string | null;
  directoryCancelled: boolean;
}

export type ShellUiAction =
  | { type: 'snapshot/replaced'; snapshot: ShellRuntimeSnapshot }
  | { type: 'settings/replaced'; settings: ShellSettingsSnapshot }
  | { type: 'transcript/loaded'; transcript: ShellTranscriptSnapshot }
  | { type: 'session/updated'; update: ShellSessionUpdate }
  | { type: 'interaction/requested'; interaction: ShellInteraction }
  | { type: 'interaction/responded'; actionId: string }
  | { type: 'sessions/loading' }
  | { type: 'sessions/loaded'; items: ShellSessionSummary[] }
  | { type: 'outputs/loading' }
  | {
      type: 'outputs/loaded';
      items: ShellArtifactSummary[];
      totalCount: number;
      truncated: boolean;
    }
  | { type: 'draft/changed'; draft: string }
  | { type: 'failure/raised'; failure: ShellOperationFailure }
  | { type: 'failure/cleared' }
  | { type: 'pending/changed'; pending: ShellUiPendingOperation | null }
  | { type: 'view/changed'; view: ShellUiView }
  | { type: 'handoff/prepared'; handoff: ShellHandoffEnvelope }
  | { type: 'handoff/cleared' }
  | { type: 'diagnostics/saved'; fileName: string }
  | { type: 'directory/cancelled' }
  | { type: 'notices/cleared' };

export function initialShellUiState(): ShellUiState {
  return {
    snapshot: null,
    settings: null,
    transcript: {
      sessionId: null,
      updates: [],
      integrity: 'complete',
      truncated: false,
      firstSeq: null,
      lastSeq: null,
      hasGap: false,
    },
    interactions: [],
    sessions: { status: 'idle', items: [] },
    outputs: { status: 'idle', items: [], totalCount: 0, truncated: false },
    draft: '',
    failure: null,
    pending: null,
    respondedActionIds: [],
    view: 'workspace',
    handoff: null,
    savedDiagnosticsFile: null,
    directoryCancelled: false,
  };
}
