type ModelContextLimit = {
  name: string;
  context_limit?: number | null;
};

export async function resolveContextLimit(
  model: string,
  predefinedModels: ModelContextLimit[],
  loadRouteModels: () => Promise<ModelContextLimit[]>,
  loadCanonicalLimit: () => Promise<number | null | undefined>,
  fallback: number
): Promise<number> {
  const predefined = predefinedModels.find((candidate) => candidate.name === model);
  if (predefined?.context_limit) return predefined.context_limit;

  const routeModel = (await loadRouteModels()).find((candidate) => candidate.name === model);
  if (routeModel?.context_limit) return routeModel.context_limit;

  return (await loadCanonicalLimit()) || fallback;
}
