import { contextBridge, ipcRenderer } from 'electron';
import type { IpcRendererEvent } from 'electron';
import { shellIpcChannels } from './ipc';
import type {
  ShellDiagnosticsSaveRequest,
  ShellGenerationRequest,
  ShellHandoffConfirmRequest,
  ShellHandoffPrepareRequest,
  ShellIpcEventMap,
  ShellIpcRequestMap,
  ShellIpcResponseMap,
} from './ipc';
import type { ShellLifecycleState } from './lifecycle';
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
    onChanged: (listener: (state: ShellLifecycleState) => void) => {
      const wrapped = (
        _event: IpcRendererEvent,
        state: ShellIpcEventMap[typeof shellIpcChannels.runtimeChanged]
      ) => listener(state);
      ipcRenderer.on(shellIpcChannels.runtimeChanged, wrapped);
      return () => ipcRenderer.removeListener(shellIpcChannels.runtimeChanged, wrapped);
    },
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
