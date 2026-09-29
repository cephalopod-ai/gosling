export const MIN_MAIN_CONTENT_WIDTH = 400;
export const MIN_DOCKED_PANE_WIDTH = 320;

export type ArtifactPaneMode = 'docked' | 'overlay';

export interface ArtifactPaneLayout {
  mode: ArtifactPaneMode;
  width: number;
}

// A docked pane gives up width before the main panel drops below its minimum.
// Once even the narrowest docked pane would crush the main panel, the pane
// floats over it instead of taking layout space.
export function resolveArtifactPaneLayout(
  windowWidth: number,
  navWidth: number,
  preferredWidth: number
): ArtifactPaneLayout {
  const room = windowWidth - navWidth - MIN_MAIN_CONTENT_WIDTH;
  if (room >= MIN_DOCKED_PANE_WIDTH) {
    return { mode: 'docked', width: Math.min(preferredWidth, room) };
  }
  return { mode: 'overlay', width: Math.min(preferredWidth, windowWidth) };
}
