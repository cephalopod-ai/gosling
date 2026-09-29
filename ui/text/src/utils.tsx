export function isErrorStatus(status: string): boolean {
  return status.startsWith("error") || status.startsWith("failed");
}

export function formatError(e: unknown): string {
  if (e instanceof Error) {
    return e.message || e.toString();
  }
  if (typeof e === "string") {
    return e;
  }
  if (e && typeof e === "object") {
    try {
      return JSON.stringify(e, null, 2);
    } catch {
      return String(e);
    }
  }
  return String(e);
}

// Ink needs raw mode on stdin; without a TTY it renders a frame and then
// crashes with a stack trace, so interactive mode must refuse up front.
export function interactiveTerminalError(
  textMode: boolean,
  stdinIsTTY: boolean,
): string | null {
  if (textMode || stdinIsTTY) return null;
  return (
    "gosling tui needs an interactive terminal (stdin is not a TTY). " +
    'For non-interactive use, run `gosling tui --text "<prompt>"` or `gosling run`.'
  );
}

export function truncateTerminalText(value: string, maxCells: number): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  const budget = Math.max(0, Math.floor(maxCells));
  if (budget === 0 || normalized.length === 0) return "";

  const chars = Array.from(normalized);
  const cellWidths = chars.map((char) =>
    char.codePointAt(0)! >= 0x20 && char.codePointAt(0)! <= 0x7e ? 1 : 2,
  );
  if (cellWidths.reduce((total, width) => total + width, 0) <= budget) {
    return normalized;
  }
  if (budget === 1) return "…";

  let used = 0;
  let result = "";
  for (let index = 0; index < chars.length; index++) {
    const width = cellWidths[index];
    if (used + width > budget - 1) break;
    result += chars[index];
    used += width;
  }
  return result + "…";
}

export function wrapTerminalText(value: string, maxCells: number): string[] {
  const budget = Math.max(1, Math.floor(maxCells));
  const lines: string[] = [];
  let current = "";
  let used = 0;

  for (const char of Array.from(value)) {
    if (char === "\n") {
      lines.push(current);
      current = "";
      used = 0;
      continue;
    }

    const width =
      char.codePointAt(0)! >= 0x20 && char.codePointAt(0)! <= 0x7e ? 1 : 2;
    if (current && used + width > budget) {
      lines.push(current);
      current = "";
      used = 0;
    }
    current += char;
    used += width;
  }

  lines.push(current);
  return lines;
}
