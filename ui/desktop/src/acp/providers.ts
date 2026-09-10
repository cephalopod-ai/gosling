import {
  zProviderTypeDto,
  type CanonicalModelInfoDto,
  type CustomProviderCreateRequest_unstable,
  type CustomProviderReadResponse_unstable,
  type ProviderSecretDto,
  type ProviderTemplateCatalogEntryDto,
  type ProviderTemplateDto,
  type SessionHandoffSnapshotV1Dto,
} from '@repo-makeover/gosling-sdk';
import type {
  ProviderDetails,
  ThinkingEffort,
  UpdateCustomProviderRequest,
} from '../types/providers';
import type { Message } from '../types/message';
import { getAcpClient } from './acpConnection';

export type { CanonicalModelInfoDto, ProviderSecretDto };

/**
 * Validates a provider type string against the set of known values instead of
 * blindly trusting the wire format. Throws loudly if the server ever sends a
 * value the client doesn't recognize (e.g. after a Rust-side enum change that
 * wasn't matched by a client update) rather than silently producing a value
 * that only *claims* to be a `ProviderDetails['provider_type']`.
 */
export function parseProviderType(value: string): ProviderDetails['provider_type'] {
  return zProviderTypeDto.parse(value);
}

function updateRequestToCreate(
  request: UpdateCustomProviderRequest
): CustomProviderCreateRequest_unstable {
  return {
    engine: request.engine,
    displayName: request.display_name,
    apiUrl: request.api_url,
    apiKey: request.api_key || null,
    models: request.models,
    supportsStreaming: request.supports_streaming ?? null,
    headers: request.headers ?? undefined,
    requiresAuth: request.requires_auth ?? true,
    catalogProviderId: request.catalog_provider_id ?? null,
    basePath: request.base_path ?? null,
    preservesThinking: request.preserves_thinking ?? null,
  };
}

let providerDetailsCache: Awaited<ReturnType<typeof fetchProviderDetails>> | undefined;
let providerDetailsInFlight: ReturnType<typeof fetchProviderDetails> | undefined;
let providerDetailsGeneration = 0;
let providerDetailsMutationDepth = 0;

export function invalidateProviderDetailsCache(): void {
  providerDetailsGeneration += 1;
  providerDetailsCache = undefined;
  providerDetailsInFlight = undefined;
}

function beginProviderDetailsMutation(): void {
  providerDetailsMutationDepth += 1;
  invalidateProviderDetailsCache();
}

function endProviderDetailsMutation(): void {
  providerDetailsMutationDepth -= 1;
  invalidateProviderDetailsCache();
}

export async function acpListProviderDetails(
  forceRefresh = false
): ReturnType<typeof fetchProviderDetails> {
  if (forceRefresh) {
    invalidateProviderDetailsCache();
  }
  if (providerDetailsMutationDepth === 0 && providerDetailsCache) {
    return providerDetailsCache;
  }
  if (providerDetailsInFlight) {
    return providerDetailsInFlight;
  }

  const generation = providerDetailsGeneration;
  const request = fetchProviderDetails();
  providerDetailsInFlight = request;
  try {
    const providers = await request;
    if (generation !== providerDetailsGeneration) {
      return acpListProviderDetails();
    }
    if (providerDetailsMutationDepth === 0) {
      providerDetailsCache = providers;
    }
    return providers;
  } finally {
    if (providerDetailsInFlight === request) {
      providerDetailsInFlight = undefined;
    }
  }
}

async function fetchProviderDetails(): Promise<ProviderDetails[]> {
  const client = await getAcpClient();
  const { entries } = await client.gosling.providersList_unstable({});
  return entries.map((entry) => ({
    name: entry.providerId,
    is_configured: entry.configured,
    manages_own_context: entry.managesOwnContext ?? false,
    capabilities: entry.capabilities ?? {
      contextOwnership: entry.managesOwnContext ? 'provider' : 'gosling',
      nativeResume: 'unsupported',
      historyImport: 'unsupported',
      inPlaceModelChange: entry.managesOwnContext ? 'unsupported' : 'supported',
      sessionFork: entry.managesOwnContext ? 'unsupported' : 'supported',
      bootstrapHandoff: entry.managesOwnContext ? 'required' : 'unsupported',
      bootstrapAcknowledgement: entry.managesOwnContext ? 'required' : 'unsupported',
    },
    provider_type: parseProviderType(entry.providerType),
    metadata: {
      name: entry.providerId,
      display_name: entry.providerName,
      description: entry.description,
      default_model: entry.defaultModel,
      model_doc_link: '',
      model_selection_hint: entry.modelSelectionHint ?? null,
      config_keys: entry.configKeys.map((key) => ({
        name: key.name,
        required: key.required,
        secret: key.secret,
        default: key.default ?? null,
        oauth_flow: key.oauthFlow ?? false,
        device_code_flow: key.deviceCodeFlow ?? false,
        primary: key.primary ?? false,
      })),
      known_models: entry.models.map((model) => ({
        name: model.id,
        context_limit: model.contextLimit ?? 0,
        reasoning: model.reasoning ?? undefined,
      })),
      setup_steps: entry.setupSteps,
    },
  }));
}

export async function acpListProviderModels(providerId: string) {
  const client = await getAcpClient();
  const [supportedModelsResult, providerDetailsResult] = await Promise.allSettled([
    client.gosling.providersSupportedModelsList_unstable({ providerId }),
    client.gosling.providersList_unstable({ providerIds: [providerId] }),
  ]);

  const providerDetails =
    providerDetailsResult.status === 'fulfilled'
      ? providerDetailsResult.value.entries.find((entry) => entry.providerId === providerId)
      : null;
  const inventoryModels = providerDetails?.models ?? [];
  const inventoryById = new Map(
    inventoryModels.map((model) => [
      model.id,
      {
        contextLimit: model.contextLimit ?? undefined,
        reasoning: model.reasoning ?? undefined,
        thinkingEfforts: model.thinkingEfforts ?? [],
      },
    ])
  );

  if (supportedModelsResult.status === 'fulfilled') {
    return supportedModelsResult.value.models.map((id) => ({
      id,
      ...inventoryById.get(id),
    }));
  }

  if (inventoryModels.length > 0) {
    return inventoryModels;
  }

  throw supportedModelsResult.reason;
}

/**
 * Lists model identifiers from an ad hoc OpenAI-compatible endpoint (e.g. a
 * local Ollama instance) rather than a registered provider — used to
 * populate the Context Summarizer's model picker from whatever endpoint the
 * user has typed in, without requiring it to be configured as a provider.
 */
export async function acpListSummarizerModels(endpoint: string): Promise<string[]> {
  const client = await getAcpClient();
  const { models } = await client.gosling.summarizerSupportedModelsList_unstable({ endpoint });
  return models;
}

export async function acpListProviderCatalogEntries(
  format?: string
): Promise<ProviderTemplateCatalogEntryDto[]> {
  const client = await getAcpClient();
  const { providers } = await client.gosling.providersCatalogList_unstable(
    format ? { format } : {}
  );
  return providers;
}

export async function acpGetProviderTemplate(providerId: string): Promise<ProviderTemplateDto> {
  const client = await getAcpClient();
  const { template } = await client.gosling.providersCatalogTemplate_unstable({ providerId });
  return template;
}

export async function acpGetCustomProvider(
  providerId: string
): Promise<CustomProviderReadResponse_unstable> {
  const client = await getAcpClient();
  return client.gosling.providersCustomRead_unstable({ providerId });
}

export async function acpCreateCustomProviderFromRequest(
  request: UpdateCustomProviderRequest
): Promise<{ provider_name: string }> {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    const response = await client.gosling.providersCustomCreate_unstable(
      updateRequestToCreate(request)
    );
    return { provider_name: response.providerId };
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpUpdateCustomProviderFromRequest(
  providerId: string,
  request: UpdateCustomProviderRequest
): Promise<void> {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    await client.gosling.providersCustomUpdate_unstable({
      providerId,
      ...updateRequestToCreate(request),
    });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpDeleteCustomProvider(providerId: string) {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    return client.gosling.providersCustomDelete_unstable({ providerId });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpReadProviderConfig(providerId: string) {
  const client = await getAcpClient();
  const { fields } = await client.gosling.providersConfigRead_unstable({ providerId });
  return fields;
}

export async function acpDeleteProviderConfig(providerId: string) {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    return client.gosling.providersConfigDelete_unstable({ providerId });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpSaveProviderConfig(
  providerId: string,
  fields: { key: string; value: string }[]
): Promise<void> {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    await client.gosling.providersConfigSave_unstable({ providerId, fields });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpAuthenticateProvider(providerId: string): Promise<void> {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    await client.gosling.providersConfigAuthenticate_unstable({ providerId });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpListProviderSecrets(): Promise<ProviderSecretDto[]> {
  const client = await getAcpClient();
  const { secrets } = await client.gosling.providersSecretsList_unstable({});
  return secrets;
}

export async function acpDeleteProviderSecret(id: string): Promise<void> {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    await client.gosling.providersSecretsDelete_unstable({ id });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpAddCustomProviderSecret(name: string, value: string): Promise<void> {
  beginProviderDetailsMutation();
  try {
    const client = await getAcpClient();
    await client.gosling.providersSecretsCustomAdd_unstable({ name, value });
  } finally {
    endProviderDetailsMutation();
  }
}

export async function acpGetCanonicalModelInfo(
  provider: string,
  model: string
): Promise<CanonicalModelInfoDto | null> {
  const client = await getAcpClient();
  const { modelInfo } = await client.gosling.providersCanonicalModelInfo_unstable({
    provider,
    model,
  });
  return modelInfo ?? null;
}

export async function acpReadDefaults(): Promise<{
  providerId: string | null;
  modelId: string | null;
}> {
  const client = await getAcpClient();
  const response = await client.gosling.defaultsRead_unstable({});
  return {
    providerId: response.providerId ?? null,
    modelId: response.modelId ?? null,
  };
}

export async function acpSaveDefaults(providerId: string, modelId?: string | null): Promise<void> {
  const client = await getAcpClient();
  await client.gosling.defaultsSave_unstable({ providerId, modelId: modelId ?? null });
}

export async function acpClearDefaults(): Promise<void> {
  const client = await getAcpClient();
  await client.gosling.defaultsClear_unstable({});
}

export async function acpReadThinkingEffort(): Promise<ThinkingEffort | null> {
  const client = await getAcpClient();
  const response = await client.gosling.preferencesRead_unstable({
    keys: ['goslingThinkingEffort'],
  });
  const value = response.values.find((v) => v.key === 'goslingThinkingEffort')?.value;
  return typeof value === 'string' ? (value as ThinkingEffort) : null;
}

export async function acpSaveThinkingEffort(effort: ThinkingEffort): Promise<void> {
  const client = await getAcpClient();
  await client.gosling.preferencesSave_unstable({
    values: [{ key: 'goslingThinkingEffort', value: effort }],
  });
}

export type AppliedSessionProviderModel = {
  thinkingEffort?: ThinkingEffort;
  providerId?: string;
  modelId?: string;
  snapshot?: SessionHandoffSnapshotV1Dto;
};

/**
 * Switch the provider, model, and thinking effort for an active session in one
 * checkpoint-backed transition.
 */
export async function acpSetSessionProviderModel(
  sessionId: string,
  providerId: string,
  modelId: string,
  thinkingEffort?: ThinkingEffort | null,
  options?: {
    confirmNewContext?: boolean;
    expectedCurrentGeneration?: number;
    expectedSourceHash?: string;
    requestParams?: Record<string, unknown> | null;
    targetContextLimit?: number | null;
  }
): Promise<AppliedSessionProviderModel> {
  const client = await getAcpClient();
  const response = await client.gosling.sessionProviderTransition_unstable({
    sessionId,
    targetProvider: providerId,
    targetModel: modelId,
    targetThinkingEffort: thinkingEffort ?? null,
    targetContextLimit: options?.targetContextLimit ?? null,
    requestParams: options?.requestParams ?? null,
    expectedCurrentGeneration: options?.expectedCurrentGeneration ?? null,
    expectedSourceHash: options?.expectedSourceHash ?? null,
    confirmNewContext: options?.confirmNewContext ?? false,
  });
  return {
    providerId: response.activeProvider,
    modelId: response.activeModel,
    thinkingEffort: thinkingEffort ?? undefined,
    snapshot: response.snapshot,
  };
}

export async function acpPreviewSessionHandoff(
  sessionId: string,
  providerId: string,
  modelId: string,
  targetContextLimit?: number | null
) {
  const client = await getAcpClient();
  return client.gosling.sessionHandoffCheckpointPreview_unstable({
    sessionId,
    targetProvider: providerId,
    targetModel: modelId,
    targetContextLimit: targetContextLimit ?? null,
  });
}

export async function acpReadSessionHandoffCheckpoint(
  sessionId: string
): Promise<SessionHandoffSnapshotV1Dto | null> {
  const client = await getAcpClient();
  const response = await client.gosling.sessionHandoffCheckpointRead_unstable({ sessionId });
  return response.snapshot ?? null;
}

export async function acpRecordSessionModelSwitch(
  sessionId: string,
  message: string
): Promise<Message> {
  const client = await getAcpClient();
  const response = await client.gosling.sessionModelSwitchRecord_unstable({
    sessionId,
    message,
  });
  return response.message as Message;
}
