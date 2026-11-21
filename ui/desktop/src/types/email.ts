export type EmailProtocol = 'imap' | 'pop';
export type EmailPreset = 'gmail' | 'purelymail' | 'custom';

export interface EmailEndpoint {
  host: string;
  port: number;
  startTls: boolean;
  username: string;
}

export interface EmailSettings {
  address: string;
  protocol: EmailProtocol;
  incoming: EmailEndpoint;
  outgoing: EmailEndpoint;
  provider: string;
  model: string;
  instructions: string;
  senders: string[];
  recipients: string[];
  folders: string[];
  archive: string;
  trash: string;
  includeExisting: boolean;
  localArchive: boolean;
  captureNotes?: boolean;
}

export interface EmailActivity {
  id: string;
  subject: string;
  status: string;
  at: string;
  draft: { recipient: string; text: string } | null;
  localBody: string | null;
  localFolder: string | null;
  taskRequest?: boolean;
  taskSessionId?: string | null;
  taskResult?: string | null;
  paper?: {
    title: string;
    pdf: boolean;
    summary: string | null;
    scope?: 'summary' | 'zotero' | 'math' | 'physics' | null;
    destinations: Partial<
      Record<
        'zotero' | 'math' | 'physics',
        {
          status: 'inProgress' | 'saved' | 'failed';
          detail?: string;
        }
      >
    >;
  } | null;
  trashed: { folder: string } | null;
}

export interface EmailSnapshot {
  revision: number;
  account: {
    settings: EmailSettings;
    enabled: boolean;
    tested: boolean;
    smtpTested: boolean;
    hasCredentials: boolean;
  } | null;
  activity: EmailActivity[];
  lastStatus: string | null;
}

export type EmailRequest =
  | { op: 'status' | 'test' | 'poll' }
  | { op: 'enable' | 'pause' | 'disconnect' | 'forget' | 'reset'; revision: number }
  | { op: 'send' | 'restore' | 'discard' | 'retryResearch'; id: string; revision: number }
  | {
      op: 'save';
      revision: number;
      settings: EmailSettings;
      incoming_password: string;
      outgoing_password: string;
    };

export function applyEmailPreset(
  settings: EmailSettings,
  preset: EmailPreset,
  protocol: EmailProtocol
): EmailSettings {
  const incomingPort = protocol === 'imap' ? 993 : 995;
  if (preset === 'custom') {
    return {
      ...settings,
      protocol,
      incoming: {
        ...settings.incoming,
        port: settings.protocol === protocol ? settings.incoming.port : incomingPort,
      },
    };
  }

  const domain = preset === 'gmail' ? 'gmail.com' : 'purelymail.com';
  const incomingService = protocol === 'imap' ? 'imap' : preset === 'gmail' ? 'pop' : 'pop3';
  return {
    ...settings,
    protocol,
    incoming: {
      host: `${incomingService}.${domain}`,
      port: incomingPort,
      startTls: false,
      username: '',
    },
    outgoing: { host: `smtp.${domain}`, port: 465, startTls: false, username: '' },
  };
}
