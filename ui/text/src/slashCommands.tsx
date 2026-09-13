import { spawnSync } from "node:child_process";

export interface SlashCommandContext {
  cwd: string;
}

export type SlashCommandResult =
  | { handled: true; message?: string }
  | { handled: true; overlay: "diff"; content: string; truncated: boolean }
  | { handled: true; exit: true }
  | { handled: false };

export interface SlashCommand {
  name: string;
  description: string;
  run: (ctx: SlashCommandContext) => SlashCommandResult;
}

function isGitRepo(cwd: string): boolean {
  const result = spawnSync("git", ["rev-parse", "--is-inside-work-tree"], {
    cwd,
    stdio: ["ignore", "ignore", "ignore"],
  });
  return result.status === 0;
}

const MAX_DIFF_BYTES = 2_000_000;

function readDiff(cwd: string): { text: string; truncated: boolean } | null {
  const result = spawnSync(
    "git",
    ["--no-pager", "diff", "--no-color"],
    {
      cwd,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
    },
  );
  if (result.status !== 0 && result.status !== null) return null;
  const stdout = result.stdout ?? "";
  if (stdout.length > MAX_DIFF_BYTES) {
    return { text: stdout.slice(0, MAX_DIFF_BYTES), truncated: true };
  }
  return { text: stdout, truncated: false };
}

const diffCommand: SlashCommand = {
  name: "diff",
  description: "show unstaged changes",
  run: (ctx) => {
    if (!isGitRepo(ctx.cwd)) {
      return {
        handled: true,
        message: `not a git repository: ${ctx.cwd}`,
      };
    }

    const diff = readDiff(ctx.cwd);
    if (diff === null) {
      return { handled: true, message: "failed to run `git diff`" };
    }

    if (diff.text.trim().length === 0) {
      return { handled: true, message: "no unstaged changes" };
    }

    return {
      handled: true,
      overlay: "diff",
      content: diff.text,
      truncated: diff.truncated,
    };
  },
};

const helpCommand: SlashCommand = {
  name: "help",
  description: "show commands and keys",
  run: () => ({ handled: true, message: helpText() }),
};

const exitCommand: SlashCommand = {
  name: "exit",
  description: "quit gosling",
  run: () => ({ handled: true, exit: true }),
};

const COMMANDS: Record<string, SlashCommand> = {
  diff: diffCommand,
  help: helpCommand,
  exit: exitCommand,
};

const ALIASES: Record<string, string> = {
  "?": "help",
  quit: "exit",
};

function helpText(): string {
  const commands = listSlashCommands()
    .map((cmd) => `- /${cmd.name} — ${cmd.description}`)
    .join("\n");
  return [
    "**Commands** (handled locally, never sent to the model)",
    "",
    commands,
    "- /quit, /? — aliases for /exit and /help",
    "",
    "**Keys**",
    "",
    "- esc / ctrl+c — cancel the running turn; when idle, quit",
    "- ctrl+c twice — quit while a turn is running",
    "- ctrl+p provider · ctrl+m model · ctrl+e extensions",
    "- shift+↑↓ turn history · ↑↓ scroll or select tool calls",
  ].join("\n");
}

export function tryRunSlashCommand(
  input: string,
  ctx: SlashCommandContext,
): SlashCommandResult {
  const trimmed = input.trim();
  if (!trimmed.startsWith("/")) return { handled: false };
  const token = trimmed.split(/\s+/)[0] ?? "";
  // `/usr/bin/foo is failing` is a prompt about a path, not a command.
  if (token.indexOf("/", 1) !== -1) return { handled: false };
  const typed = token.slice(1).toLowerCase();
  const cmd = COMMANDS[ALIASES[typed] ?? typed];
  if (!cmd) {
    return {
      handled: true,
      message: `unknown command ${token} — type /help for the list of commands`,
    };
  }
  return cmd.run(ctx);
}

export function listSlashCommands(): SlashCommand[] {
  return Object.values(COMMANDS);
}
