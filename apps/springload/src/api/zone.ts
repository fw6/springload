import { invoke } from "./invoke";
import type { SupportZoneView } from "./types";

/** 图上可以落成一条记录的标注类型。AVWAP 起点由图表视图状态承载。 */
export type ZoneKind = "hline" | "support" | "gap";

/** `support_zone_save` 的入参。`id` 为空时由服务端生成。 */
export interface SupportZoneInput {
  id?: string;
  code: string;
  kind: ZoneKind;
  low: number;
  high: number;
  anchorDate?: string | null;
  avwapStartDate?: string | null;
}

/** `support_refs` 命令的返回：三个参考价，数据不足时为 null。 */
export interface ZoneRefs {
  ema21: number | null;
  avwap: number | null;
  prevHigh: number | null;
}

/** 列出某只股票的标注。 */
export function supportZonesGet(code: string): Promise<SupportZoneView[]> {
  return invoke<SupportZoneView[]>("support_zones_get", { code });
}

/** 写入或更新一条标注，服务端按当前参考价重算置信度。 */
export function supportZoneSave(zone: SupportZoneInput): Promise<SupportZoneView> {
  return invoke<SupportZoneView>("support_zone_save", { zone });
}

/** 删除一条标注。 */
export function supportZoneDelete(id: string): Promise<void> {
  return invoke<void>("support_zone_delete", { id });
}

/** 三个参考价。`avwapStartDate` 为空时服务端按默认起点算 AVWAP。 */
export function supportRefs(code: string, avwapStartDate: string | null): Promise<ZoneRefs> {
  return invoke<ZoneRefs>("support_refs", { code, avwapStartDate });
}
