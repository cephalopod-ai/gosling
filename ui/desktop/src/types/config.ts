export type ConfigValue =
  | string
  | number
  | boolean
  | null
  | undefined
  | ConfigValue[]
  | ConfigObject;
export type ConfigObject = { [key: string]: ConfigValue };
export type ConfigData = Record<string, ConfigValue>;
