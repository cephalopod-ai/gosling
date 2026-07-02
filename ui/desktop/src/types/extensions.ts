export type Envs = Record<string, string>;

type BuiltinExtensionConfig = {
  name: string;
  description?: string | null;
  display_name?: string | null;
  timeout?: number | null;
  bundled?: boolean | null;
  available_tools?: string[] | null;
  type: 'builtin';
};

type PlatformExtensionConfig = {
  name: string;
  description?: string | null;
  display_name?: string | null;
  bundled?: boolean | null;
  available_tools?: string[] | null;
  type: 'platform';
};

type StdioExtensionConfig = {
  name: string;
  description?: string | null;
  cmd: string;
  args?: string[];
  envs?: Envs;
  env_keys?: string[];
  timeout?: number | null;
  cwd?: string | null;
  bundled?: boolean | null;
  available_tools?: string[] | null;
  type: 'stdio';
};

type StreamableHttpExtensionConfig = {
  name: string;
  description?: string | null;
  uri: string;
  envs?: Envs;
  env_keys?: string[];
  headers?: Record<string, string>;
  timeout?: number | null;
  socket?: string | null;
  bundled?: boolean | null;
  available_tools?: string[] | null;
  type: 'streamable_http';
};

type LegacySseExtensionConfig = {
  description?: string | null;
  name: string;
  type: 'sse';
  uri?: string | null;
};

type FrontendTool = {
  _meta?: Record<string, unknown>;
  annotations?: Record<string, unknown>;
  description?: string;
  execution?: Record<string, unknown>;
  icons?: unknown[];
  inputSchema: Record<string, unknown>;
  name: string;
  outputSchema?: Record<string, unknown>;
  title?: string;
};

type FrontendExtensionConfig = {
  available_tools?: string[] | null;
  bundled?: boolean | null;
  description?: string | null;
  instructions?: string | null;
  name: string;
  tools: FrontendTool[];
  type: 'frontend';
};

type InlinePythonExtensionConfig = {
  available_tools?: string[] | null;
  code: string;
  dependencies?: string[] | null;
  description?: string | null;
  name: string;
  timeout?: number | null;
  type: 'inline_python';
};

export type ExtensionConfig =
  | BuiltinExtensionConfig
  | PlatformExtensionConfig
  | StdioExtensionConfig
  | StreamableHttpExtensionConfig
  | LegacySseExtensionConfig
  | FrontendExtensionConfig
  | InlinePythonExtensionConfig;

export type ExtensionEntry = ExtensionConfig & {
  enabled: boolean;
};

export type ExtensionLoadResult = {
  error?: string | null;
  name: string;
  success: boolean;
};
