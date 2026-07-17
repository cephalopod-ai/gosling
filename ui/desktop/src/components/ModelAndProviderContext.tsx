import React, { createContext, useContext, useState, useEffect, useMemo, useCallback } from 'react';
import { toastError, toastSuccess } from '../toasts';
import Model, { getProviderMetadata } from './settings/models/modelInterface';
import type { ProviderMetadata } from '../types/providers';
import { acpChatSessionActions, acpChatSessionStore } from '../acp/chatSessionStore';
import {
  acpReadDefaults,
  acpRecordSessionModelSwitch,
  acpSaveDefaults,
  acpSetSessionProviderModel,
  type AppliedSessionProviderModel,
} from '../acp/providers';
import { errorMessage } from '../utils/conversionUtils';
import {
  getModelDisplayName,
  getProviderDisplayName,
} from './settings/models/predefinedModelsUtils';
import { defineMessages, useIntl } from '../i18n';
import type { Message } from '../types/message';
import type { ThinkingEffort } from '../types/providers';
import type { Session } from '../types/session';

const i18n = defineMessages({
  unknownProviderTitle: {
    id: 'modelAndProviderContext.unknownProviderTitle',
    defaultMessage: 'Provider name lookup',
  },
  unknownProviderMsg: {
    id: 'modelAndProviderContext.unknownProviderMsg',
    defaultMessage: 'Unknown provider in config -- please inspect your config.yaml',
  },
  modelChangedTitle: {
    id: 'modelAndProviderContext.modelChangedTitle',
    defaultMessage: 'Model changed',
  },
  switchModelSuccess: {
    id: 'modelAndProviderContext.switchModelSuccess',
    defaultMessage: 'Successfully switched models -- using {model} from {provider}',
  },
  modelSwitchRecord: {
    id: 'modelAndProviderContext.modelSwitchRecord',
    defaultMessage: 'Model changed: {previousModel} -> {currentModel}',
  },
  modelChangeFailed: {
    id: 'modelAndProviderContext.modelChangeFailed',
    defaultMessage: '{provider}/{model} failed',
  },
  selectModel: {
    id: 'modelAndProviderContext.selectModel',
    defaultMessage: 'Select Model',
  },
});

interface ModelAndProviderContextType {
  currentModel: string | null;
  currentProvider: string | null;
  changeModel: (sessionId: string | null, model: Model) => Promise<boolean>;
  getCurrentModelAndProvider: () => Promise<{ model: string; provider: string }>;
  getFallbackModelAndProvider: () => Promise<{ model: string; provider: string }>;
  getCurrentModelAndProviderForDisplay: () => Promise<{ model: string; provider: string }>;
  getCurrentModelDisplayName: () => Promise<string>;
  getCurrentProviderDisplayName: () => Promise<string>; // Gets provider display name from subtext
  refreshCurrentModelAndProvider: () => Promise<void>;
}

interface ModelAndProviderProviderProps {
  children: React.ReactNode;
}

const ModelAndProviderContext = createContext<ModelAndProviderContextType | undefined>(undefined);

export { i18n as modelAndProviderMessages };

function patchAcpSessionProviderModel(
  sessionId: string,
  { providerId, modelId, thinkingEffort }: AppliedSessionProviderModel
) {
  if (!providerId && !modelId && !thinkingEffort) return;

  const currentSession = acpChatSessionStore.getSnapshot(sessionId)?.session;
  if (!currentSession) return;

  const nextModelConfig: NonNullable<Session['model_config']> | undefined =
    currentSession.model_config
      ? { ...currentSession.model_config }
      : modelId
        ? { model_name: modelId, toolshim: false }
        : undefined;

  if (nextModelConfig) {
    if (modelId) {
      nextModelConfig.model_name = modelId;
    }
    if (thinkingEffort) {
      nextModelConfig.request_params = {
        ...(nextModelConfig.request_params ?? {}),
        thinking_effort: thinkingEffort,
      };
    }
  }

  acpChatSessionActions.setSessionMetadata(sessionId, {
    ...currentSession,
    provider_name: providerId ?? currentSession.provider_name,
    model_config: nextModelConfig ?? currentSession.model_config,
  });
}

const THINKING_EFFORTS = new Set<ThinkingEffort>(['off', 'low', 'medium', 'high', 'max', 'ultra']);

function parseThinkingEffort(value: unknown): ThinkingEffort | undefined {
  return typeof value === 'string' && THINKING_EFFORTS.has(value as ThinkingEffort)
    ? (value as ThinkingEffort)
    : undefined;
}

function formatThinkingEffort(effort: ThinkingEffort | undefined): string | undefined {
  if (!effort || effort === 'off') return undefined;
  return effort.charAt(0).toUpperCase() + effort.slice(1);
}

function formatModelSelectionLabel(input: {
  providerId?: string | null;
  providerDisplayName?: string | null;
  modelId?: string | null;
  modelDisplayName?: string | null;
  thinkingEffort?: ThinkingEffort;
}): string | null {
  const provider = input.providerDisplayName || input.providerId || '';
  const model = input.modelDisplayName || input.modelId || '';
  const effort = formatThinkingEffort(input.thinkingEffort);
  const parts = [provider, model, effort].filter((part): part is string => Boolean(part));
  return parts.length > 0 ? parts.join(' ') : null;
}

function sessionModelSelectionLabel(session: Session | undefined): string | null {
  const modelId = session?.model_config?.model_name;
  const providerId = session?.provider_name;
  return formatModelSelectionLabel({
    providerId,
    providerDisplayName: modelId ? getProviderDisplayName(modelId) : undefined,
    modelId,
    modelDisplayName: modelId ? getModelDisplayName(modelId) : undefined,
    thinkingEffort: parseThinkingEffort(session?.model_config?.request_params?.thinking_effort),
  });
}

function selectedModelSelectionLabel(model: Model): string {
  return (
    formatModelSelectionLabel({
      providerId: model.provider,
      providerDisplayName: model.subtext || getProviderDisplayName(model.name),
      modelId: model.name,
      modelDisplayName: model.alias || getModelDisplayName(model.name),
      thinkingEffort: parseThinkingEffort(model.request_params?.thinking_effort),
    }) ?? model.name
  );
}

function appendStoredMessage(sessionId: string, message: Message) {
  const snapshot = acpChatSessionStore.getSnapshot(sessionId);
  if (
    !snapshot ||
    snapshot.messages.some((existing) => existing.id && existing.id === message.id)
  ) {
    return;
  }
  acpChatSessionActions.setMessages(sessionId, [...snapshot.messages, message]);
}

export const ModelAndProviderProvider: React.FC<ModelAndProviderProviderProps> = ({ children }) => {
  const [currentModel, setCurrentModel] = useState<string | null>(null);
  const [currentProvider, setCurrentProvider] = useState<string | null>(null);
  const intl = useIntl();

  const changeModel = useCallback(
    async (sessionId: string | null, model: Model) => {
      const modelName = model.name;
      const providerName = model.provider;
      let phase = 'agent';
      const previousModelLabel = sessionId
        ? sessionModelSelectionLabel(acpChatSessionStore.getSnapshot(sessionId)?.session)
        : null;

      try {
        if (sessionId) {
          const applied = await acpSetSessionProviderModel(
            sessionId,
            providerName,
            modelName,
            model.request_params?.thinking_effort ?? null
          );
          patchAcpSessionProviderModel(sessionId, applied);

          const modelForRecord: Model = {
            ...model,
            request_params: {
              ...model.request_params,
              ...(applied.thinkingEffort ? { thinking_effort: applied.thinkingEffort } : {}),
            },
          };
          const currentModelLabel = selectedModelSelectionLabel(modelForRecord);
          if (previousModelLabel && previousModelLabel !== currentModelLabel) {
            try {
              const storedMessage = await acpRecordSessionModelSwitch(
                sessionId,
                intl.formatMessage(i18n.modelSwitchRecord, {
                  previousModel: previousModelLabel,
                  currentModel: currentModelLabel,
                })
              );
              appendStoredMessage(sessionId, storedMessage);
            } catch (error) {
              console.warn('Failed to record model switch:', error);
            }
          }
        }

        // Only update the global config default when there's no session
        // (i.e. changing from settings, not from within an existing chat)
        if (!sessionId) {
          phase = 'config';
          await acpSaveDefaults(providerName, modelName);
        }

        if (!sessionId) {
          setCurrentProvider(providerName);
          setCurrentModel(modelName);
        }

        toastSuccess({
          title: intl.formatMessage(i18n.modelChangedTitle),
          msg: intl.formatMessage(i18n.switchModelSuccess, {
            model: model.alias ?? modelName,
            provider: model.subtext ?? providerName,
          }),
        });
        return true;
      } catch (error) {
        console.error(`Failed to change model at ${phase} step -- ${modelName} ${providerName}`);
        toastError({
          title: intl.formatMessage(i18n.modelChangeFailed, {
            provider: providerName,
            model: modelName,
          }),
          msg: `${error}`,
          traceback: errorMessage(error),
        });
        return false;
      }
    },
    [intl]
  );

  const getFallbackModelAndProvider = useCallback(async () => {
    const provider = window.appConfig.get('GOSLING_DEFAULT_PROVIDER') as string;
    const model = window.appConfig.get('GOSLING_DEFAULT_MODEL') as string;
    if (provider && model) {
      try {
        await acpSaveDefaults(provider, model);
      } catch (error) {
        console.error('[getFallbackModelAndProvider] Failed to write to config', error);
      }
    }
    return { model: model, provider: provider };
  }, []);

  const getCurrentModelAndProvider = useCallback(async () => {
    let model: string | null;
    let provider: string | null;

    try {
      const defaults = await acpReadDefaults();
      model = defaults.modelId;
      provider = defaults.providerId;
    } catch {
      console.error(`Failed to read default model or provider`);
      throw new Error('Failed to read default model or provider');
    }
    if (!model || !provider) {
      return getFallbackModelAndProvider();
    }
    return { model: model, provider: provider };
  }, [getFallbackModelAndProvider]);

  const getCurrentModelAndProviderForDisplay = useCallback(async () => {
    const modelProvider = await getCurrentModelAndProvider();
    const goslingModel = modelProvider.model;
    const goslingProvider = modelProvider.provider;

    // lookup display name
    let metadata: ProviderMetadata;

    try {
      metadata = await getProviderMetadata(String(goslingProvider));
    } catch {
      return { model: goslingModel, provider: goslingProvider };
    }
    const providerDisplayName = metadata.display_name;

    return { model: goslingModel, provider: providerDisplayName };
  }, [getCurrentModelAndProvider]);

  const getCurrentModelDisplayName = useCallback(async () => {
    try {
      const { modelId } = await acpReadDefaults();
      return getModelDisplayName(modelId ?? '');
    } catch {
      return intl.formatMessage(i18n.selectModel);
    }
  }, [intl]);

  const getCurrentProviderDisplayName = useCallback(async () => {
    try {
      const { modelId } = await acpReadDefaults();
      const providerDisplayName = getProviderDisplayName(modelId ?? '');
      if (providerDisplayName) {
        return providerDisplayName;
      }
      // Fall back to regular provider display name lookup
      const { provider } = await getCurrentModelAndProviderForDisplay();
      return provider;
    } catch {
      return '';
    }
  }, [getCurrentModelAndProviderForDisplay]);

  const refreshCurrentModelAndProvider = useCallback(async () => {
    try {
      const { model, provider } = await getCurrentModelAndProvider();
      setCurrentModel(model);
      setCurrentProvider(provider);
    } catch (_error) {
      console.error('Failed to refresh current model and provider:', _error);
    }
  }, [getCurrentModelAndProvider]);

  // Load initial model and provider on mount
  useEffect(() => {
    refreshCurrentModelAndProvider();
  }, [refreshCurrentModelAndProvider]);

  const contextValue = useMemo(
    () => ({
      currentModel,
      currentProvider,
      changeModel,
      getCurrentModelAndProvider,
      getFallbackModelAndProvider,
      getCurrentModelAndProviderForDisplay,
      getCurrentModelDisplayName,
      getCurrentProviderDisplayName,
      refreshCurrentModelAndProvider,
    }),
    [
      currentModel,
      currentProvider,
      changeModel,
      getCurrentModelAndProvider,
      getFallbackModelAndProvider,
      getCurrentModelAndProviderForDisplay,
      getCurrentModelDisplayName,
      getCurrentProviderDisplayName,
      refreshCurrentModelAndProvider,
    ]
  );

  return (
    <ModelAndProviderContext.Provider value={contextValue}>
      {children}
    </ModelAndProviderContext.Provider>
  );
};

export const useModelAndProvider = () => {
  const context = useContext(ModelAndProviderContext);
  if (context === undefined) {
    throw new Error('useModelAndProvider must be used within a ModelAndProviderProvider');
  }
  return context;
};
