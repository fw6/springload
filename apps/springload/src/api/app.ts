import { invoke } from "./invoke";

/** 支持的市场。 */
export type Market = "cn" | "us";

/** `app_info` 命令的返回。 */
export interface AppInfo {
  version: string;
  /** `cn` 是 A 股，`us` 是美股。 */
  market: Market;
  /** 界面上的市场名。 */
  marketLabel: string;
  /** 计价货币名。 */
  currency: string;
  /** 是否当日买入次日才能卖出。 */
  tPlusOne: boolean;
  dataSource: string;
  lastSyncAt: string | null;
}

/** 版本号、当前市场、数据源名称与最近同步时间。 */
export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
