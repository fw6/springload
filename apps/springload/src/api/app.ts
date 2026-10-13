import { invoke } from "./invoke";

/** `app_info` 命令的返回。 */
export interface AppInfo {
  version: string;
  dataSource: string;
  lastSyncAt: string | null;
}

/** 版本号、数据源名称与最近同步时间。 */
export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
