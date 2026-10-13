import { Marked } from "marked";
import { markedTerminal } from "marked-terminal";

let renderer: Marked | null = null;
let rendererWidth = 0;

function getRenderer(width: number): Marked {
  if (renderer && rendererWidth === width) return renderer;
  renderer = new Marked();
  renderer.use(markedTerminal({ width, reflowText: true, tab: 2 }) as any);
  rendererWidth = width;
  return renderer;
}

// Callers re-render every content_chunk item on each streamed token because
// they're re-parsing the item's fully accumulated text from scratch (see
// renderContentItem in components/ContentRenderers.tsx). Only the item
// currently receiving tokens actually has new text on a given render; every
// earlier item's text is unchanged, so caching by (key, width, src) turns
// those into hits and collapses the per-render cost back to O(new text)
// instead of O(total conversation text).
const parseCache = new Map<
  string,
  { width: number; src: string; lines: string[] }
>();
const MAX_CACHE_ENTRIES = 200;
const MAX_CACHE_CHARACTERS = 2 * 1024 * 1024;
let cacheCharacters = 0;

function cachedCharacters(entry: { src: string; lines: string[] }): number {
  return (
    entry.src.length +
    entry.lines.reduce((total, line) => total + line.length, 0)
  );
}

export function renderMarkdown(
  src: string,
  width = 76,
  cacheKey?: string,
): string[] {
  if (!src) return [];

  if (cacheKey !== undefined) {
    const cached = parseCache.get(cacheKey);
    if (cached && cached.width === width && cached.src === src) {
      parseCache.delete(cacheKey);
      parseCache.set(cacheKey, cached);
      return cached.lines;
    }
  }

  const m = getRenderer(width);
  const rendered = (m.parse(src) as string).replace(/\n+$/, "");
  const lines = rendered.split("\n");

  if (cacheKey !== undefined) {
    const previous = parseCache.get(cacheKey);
    if (previous) cacheCharacters -= cachedCharacters(previous);
    parseCache.delete(cacheKey);
    const entry = { width, src, lines };
    const characters = cachedCharacters(entry);
    if (characters <= MAX_CACHE_CHARACTERS) {
      parseCache.set(cacheKey, entry);
      cacheCharacters += characters;
    }
    while (
      parseCache.size > MAX_CACHE_ENTRIES ||
      cacheCharacters > MAX_CACHE_CHARACTERS
    ) {
      const oldest = parseCache.keys().next().value!;
      cacheCharacters -= cachedCharacters(parseCache.get(oldest)!);
      parseCache.delete(oldest);
    }
  }

  return lines;
}
