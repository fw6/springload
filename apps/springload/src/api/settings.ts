import { invoke } from "./invoke";

/** 全局配置，字段与 `~/.springload/springload.config.json` 一致。 */
export interface Settings {
  riskPct: number;
  stopDistanceWarn: number;
  liquidityThreshold: number;
  chg6mThreshold: number;
  rankBand: [number, number];
  convergenceThreshold: number;
  beginnerMode: boolean;
  autostart: boolean;
  feeModelEnabled: boolean;
  commissionRate: number;
  firstRunConfirmed: boolean;
}

/** 只带要改的字段。 */
export type SettingsPatch = Partial<Settings>;

/** 读当前配置。 */
export function getSettings(): Promise<Settings> {
  return invoke<Settings>("settings_get");
}

/** 改配置里的一部分字段，返回改完的完整配置。 */
export function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return invoke<Settings>("settings_update", { patch });
}
