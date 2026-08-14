import { contextBridge, ipcRenderer } from 'electron';
import type { IpcRendererEvent } from 'electron';
import { shellIpcChannels } from './ipc';
import type {
  ShellDiagnosticsSaveRequest,
  ShellDomainActionConfirmRequest,
  ShellDomainActionRequest,
  ShellDomainSnapshotRequest,
  ShellGenerationRequest,
  ShellHandoffConfirmRequest,
  ShellHandoffPrepareRequest,
  ShellIpcEventMap,
  ShellIpcRequestMap,
  ShellIpcResponseMap,
  ShellPromptCancelRequest,
  ShellPromptSubmitRequest,
  ShellPermissionRespondRequest,
  ShellElicitationRespondRequest,
  ShellSessionResumeRequest,
} from './ipc';
import type { ShellRuntimeSnapshot } from './runtimeSnapshot';
import type { ShellSessionUpdate } from './sessionController';
import type { ShellInteraction } from './interactionController';
import type { GoslingShellAPI } from './preloadApi';

function invoke<T extends keyof ShellIpcRequestMap>(
  channel: T,
  request: ShellIpcRequestMap[T]
): Promise<ShellIpcResponseMap[T]> {
  return request === undefined ? ipcRenderer.invoke(channel) : ipcRenderer.invoke(channel, request);
}

export const goslingShellAPI: GoslingShellAPI = Object.freeze({
  runtime: Object.freeze({
    read: () => invoke(shellIpcChannels.runtimeRead, undefined),
    retry: (request: ShellGenerationRequest) => invoke(shellIpcChannels.runtimeRetry, request),
    stop: (request: ShellGenerationRequest) => invoke(shellIpcChannels.runtimeStop, request),
    onChanged: (listener: (state: ShellRuntimeSnapshot) => void) => {
      const wrapped = (
        _event: IpcRendererEvent,
        state: ShellIpcEventMap[typeof shellIpcChannels.runtimeChanged]
      ) => listener(state);
      ipcRenderer.on(shellIpcChannels.runtimeChanged, wrapped);
      return () => ipcRenderer.removeListener(shellIpcChannels.runtimeChanged, wrapped);
    },
  }),
  session: Object.freeze({
    create: (request: ShellGenerationRequest) => invoke(shellIpcChannels.sessionCreate, request),
    resume: (request: ShellSessionResumeRequest) => invoke(shellIpcChannels.sessionResume, request),
    onUpdated: (listener: (update: ShellSessionUpdate) => void) => {
      const wrapped = (
        _event: IpcRendererEvent,
        update: ShellIpcEventMap[typeof shellIpcChannels.sessionUpdated]
      ) => listener(update);
      ipcRenderer.on(shellIpcChannels.sessionUpdated, wrapped);
      return () => ipcRenderer.removeListener(shellIpcChannels.sessionUpdated, wrapped);
    },
  }),
  prompt: Object.freeze({
    submit: (request: ShellPromptSubmitRequest) => invoke(shellIpcChannels.promptSubmit, request),
    cancel: (request: ShellPromptCancelRequest) => invoke(shellIpcChannels.promptCancel, request),
  }),
  permission: Object.freeze({
    respond: (request: ShellPermissionRespondRequest) =>
      invoke(shellIpcChannels.permissionRespond, request),
    onRequested: (
      listener: (interaction: Extract<ShellInteraction, { kind: 'permission' }>) => void
    ) => {
      const wrapped = (
        _event: IpcRendererEvent,
        interaction: ShellIpcEventMap[typeof shellIpcChannels.permissionRequested]
      ) => listener(interaction);
      ipcRenderer.on(shellIpcChannels.permissionRequested, wrapped);
      return () => ipcRenderer.removeListener(shellIpcChannels.permissionRequested, wrapped);
    },
  }),
  elicitation: Object.freeze({
    respond: (request: ShellElicitationRespondRequest) =>
      invoke(shellIpcChannels.elicitationRespond, request),
    onRequested: (
      listener: (interaction: Extract<ShellInteraction, { kind: 'elicitation' }>) => void
    ) => {
      const wrapped = (
        _event: IpcRendererEvent,
        interaction: ShellIpcEventMap[typeof shellIpcChannels.elicitationRequested]
      ) => listener(interaction);
      ipcRenderer.on(shellIpcChannels.elicitationRequested, wrapped);
      return () => ipcRenderer.removeListener(shellIpcChannels.elicitationRequested, wrapped);
    },
  }),
  domain: Object.freeze({
    snapshot: (request: ShellDomainSnapshotRequest) =>
      invoke(shellIpcChannels.domainSnapshot, request),
    action: (request: ShellDomainActionRequest) => invoke(shellIpcChannels.domainAction, request),
    confirm: (request: ShellDomainActionConfirmRequest) =>
      invoke(shellIpcChannels.confirmationRespond, request),
  }),
  diagnostics: Object.freeze({
    save: (request: ShellDiagnosticsSaveRequest) =>
      invoke(shellIpcChannels.diagnosticsSave, request),
  }),
  handoff: Object.freeze({
    prepare: (request: ShellHandoffPrepareRequest) =>
      invoke(shellIpcChannels.handoffPrepare, request),
    confirm: (request: ShellHandoffConfirmRequest) =>
      invoke(shellIpcChannels.handoffConfirm, request),
  }),
  external: Object.freeze({
    open: (url: string) => invoke(shellIpcChannels.externalOpen, url),
  }),
});

contextBridge.exposeInMainWorld('goslingShell', goslingShellAPI);
