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
const parseCache = new Map<string, { width: number; src: string; lines: string[] }>();

export function renderMarkdown(src: string, width = 76, cacheKey?: string): string[] {
  if (!src) return [];

  if (cacheKey !== undefined) {
    const cached = parseCache.get(cacheKey);
    if (cached && cached.width === width && cached.src === src) {
      return cached.lines;
    }
  }

  const m = getRenderer(width);
  const rendered = (m.parse(src) as string).replace(/\n+$/, "");
  const lines = rendered.split("\n");

  if (cacheKey !== undefined) {
    parseCache.set(cacheKey, { width, src, lines });
  }

  return lines;
}
