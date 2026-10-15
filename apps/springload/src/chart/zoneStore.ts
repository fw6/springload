import { toMessage } from "../api/invoke";
import type { SupportZoneView } from "../api/types";
import {
  supportZoneDelete,
  supportZoneSave,
  supportZonesGet,
  type SupportZoneInput,
  type ZoneKind,
} from "../api/zone";

/**
 * 第 2 步标注的外部存储。
 *
 * 图表区（`KLineChart`）与面板（`Step2`）由 `FlowPage` 并列渲染，两者需要看到
 * 同一份标注列表：图上增删或拖动之后，面板的列表、置信度与 AI 卡都要跟着变。
 * 这里用一个模块级的订阅存储承载这份状态，两个组件都订阅它。
 */
export interface ZoneStoreSnapshot {
  /** 当前挂载的股票代码。 */
  code: string | null;
  zones: SupportZoneView[];
  /** 最近一次读写失败的可展示文案。 */
  error: string | null;
}

let snapshot: ZoneStoreSnapshot = { code: null, zones: [], error: null };
/** 正在取数的股票代码，避免两个组件同时触发重复请求。 */
let loading: string | null = null;
const listeners = new Set<() => void>();

function emit(next: ZoneStoreSnapshot): void {
  snapshot = next;
  for (const listener of listeners) listener();
}

export function subscribeZones(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getZones(): ZoneStoreSnapshot {
  return snapshot;
}

/** 清空标注。切换市场时调用：另一个市场的标注不该留在这里。 */
export function resetZones(): void {
  loading = null;
  emit({ code: null, zones: [], error: null });
}

/** 换股票时清空并重新取标注。 */
export async function loadZones(code: string): Promise<void> {
  if (snapshot.code !== code) emit({ code, zones: [], error: null });
  if (loading === code) return;
  loading = code;
  try {
    const zones = await supportZonesGet(code);
    if (snapshot.code === code) emit({ code, zones, error: null });
  } catch (error) {
    if (snapshot.code === code) emit({ code, zones: [], error: toMessage(error) });
  } finally {
    if (loading === code) loading = null;
  }
}

/** 写入或更新一条标注，成功后就地替换列表里的同一条。 */
export async function saveZone(input: SupportZoneInput): Promise<void> {
  try {
    const saved = await supportZoneSave(input);
    if (snapshot.code !== saved.code) return;
    const index = snapshot.zones.findIndex((zone) => zone.id === saved.id);
    const zones =
      index >= 0
        ? snapshot.zones.map((zone) => (zone.id === saved.id ? saved : zone))
        : snapshot.zones.concat(saved);
    emit({ code: saved.code, zones, error: null });
  } catch (error) {
    const code = snapshot.code;
    if (code === null) return;
    const message = toMessage(error);
    emit({ code, zones: snapshot.zones, error: message });
    // 保存失败时按数据库里的位置重新对账，图上不会停在没写进去的位置。
    const zones = await supportZonesGet(code).catch(() => snapshot.zones);
    if (snapshot.code === code) emit({ code, zones, error: message });
  }
}

/** 换 AVWAP 起点后重算所有标注的置信度。 */
export async function resaveZones(avwapStartDate: string | null): Promise<void> {
  const zones = snapshot.zones.filter(
    (zone) => zone.kind === "hline" || zone.kind === "support" || zone.kind === "gap",
  );
  for (const zone of zones) {
    await saveZone({
      id: zone.id,
      code: zone.code,
      kind: zone.kind as ZoneKind,
      low: zone.low,
      high: zone.high,
      avwapStartDate,
    });
  }
}

/** 删除一条标注。 */
export async function removeZone(id: string): Promise<void> {
  const code = snapshot.code;
  try {
    await supportZoneDelete(id);
    if (code !== null && snapshot.code === code) {
      emit({ code, zones: snapshot.zones.filter((zone) => zone.id !== id), error: null });
    }
  } catch (error) {
    if (code !== null && snapshot.code === code) {
      emit({ code, zones: snapshot.zones, error: toMessage(error) });
    }
  }
}
