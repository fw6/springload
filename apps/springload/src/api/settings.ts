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

/** `import_bars_csv` 命令的返回：写入行数、跳过行数与逐条错误。 */
export interface ImportReport {
  rows: number;
  skipped: number;
  errors: string[];
}

/** 读当前配置。 */
export function getSettings(): Promise<Settings> {
  return invoke<Settings>("settings_get");
}

/** 改配置里的一部分字段，返回改完的完整配置。 */
export function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return invoke<Settings>("settings_update", { patch });
}

/** 记下首次启动的风险提示已读，返回改完的配置。 */
export function confirmFirstRun(): Promise<Settings> {
  return invoke<Settings>("confirm_first_run");
}

/** 把一个 CSV 文件里的日线增量导入 `bar` 表。 */
export function importBarsCsv(path: string): Promise<ImportReport> {
  return invoke<ImportReport>("import_bars_csv", { path });
}
