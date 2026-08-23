import type { RecentModel } from './settings';

const MAX_RECENT_MODELS = 5;

export function addToRecentModels(
  current: RecentModel[],
  provider: string,
  model: string
): RecentModel[] {
  const filtered = current.filter(
    (recent) => !(recent.provider === provider && recent.model === model)
  );
  return [{ provider, model }, ...filtered].slice(0, MAX_RECENT_MODELS);
}
