import type { ShellHandoffEnvelope } from '@repo-makeover/gosling-sdk';
import { parseShellHandoffUri } from './shell/handoff';

export type GoslingProtocolRoute =
  | { action: 'handoff'; envelope: ShellHandoffEnvelope }
  | { action: 'new-session'; prompt?: string }
  | { action: 'resume'; sessionId: string }
  | { action: 'renderer'; kind: 'extension' | 'sessions' };

export interface FullGoslingProtocolChatOptions {
  initialMessage?: string;
  initialMessageNoAutoSubmit?: boolean;
}

export interface FullGoslingProtocolOperations {
  openChat(options: FullGoslingProtocolChatOptions): Promise<void> | void;
  resume(sessionId: string): Promise<void> | void;
  renderer(kind: 'extension' | 'sessions'): Promise<void> | void;
}

function decodeSessionId(pathname: string): string | null {
  try {
    const sessionId = decodeURIComponent(pathname.replace(/^\/+/, '')).trim();
    return sessionId || null;
  } catch {
    return null;
  }
}

export function findGoslingProtocolUrl(commandLine: readonly string[]): string | null {
  return commandLine.find((argument) => argument.startsWith('gosling://')) ?? null;
}

export function parseGoslingProtocolRoute(value: string): GoslingProtocolRoute | null {
  if (typeof value !== 'string' || !value.startsWith('gosling://')) {
    return null;
  }
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    return null;
  }
  if (parsed.protocol !== 'gosling:' || parsed.username || parsed.password) {
    return null;
  }

  switch (parsed.hostname) {
    case 'handoff': {
      const envelope = parseShellHandoffUri(value);
      return envelope ? { action: 'handoff', envelope } : null;
    }
    case 'new-session':
      return { action: 'new-session', prompt: parsed.searchParams.get('prompt') || undefined };
    case 'resume': {
      const sessionId = decodeSessionId(parsed.pathname);
      return sessionId ? { action: 'resume', sessionId } : null;
    }
    case 'extension':
    case 'sessions':
      return { action: 'renderer', kind: parsed.hostname };
    default:
      return null;
  }
}

export function formatShellHandoffDraft(envelope: ShellHandoffEnvelope): string {
  return [
    'Shell handoff received. Review the exact envelope below before sending.',
    'Receiving this draft does not grant the claimed capability, mutation authority, reference access, or return navigation.',
    '',
    JSON.stringify(envelope, null, 2),
  ].join('\n');
}

export async function dispatchFullGoslingProtocolUrl(
  value: string,
  operations: FullGoslingProtocolOperations
): Promise<boolean> {
  const route = parseGoslingProtocolRoute(value);
  if (!route) return false;
  switch (route.action) {
    case 'handoff':
      await operations.openChat({
        initialMessage: formatShellHandoffDraft(route.envelope),
        initialMessageNoAutoSubmit: true,
      });
      break;
    case 'new-session':
      await operations.openChat({
        initialMessage: route.prompt,
        initialMessageNoAutoSubmit: route.prompt !== undefined,
      });
      break;
    case 'resume':
      await operations.resume(route.sessionId);
      break;
    case 'renderer':
      await operations.renderer(route.kind);
      break;
  }
  return true;
}
