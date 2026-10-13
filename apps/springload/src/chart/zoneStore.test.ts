import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import type { SupportZoneView } from "../api/types";
import { mockedInvoke, stubInvoke } from "../test-utils";
import { getZones, loadZones, removeZone, saveZone } from "./zoneStore";

function zone(id: string, code: string, low = 9.5, high = 10.5): SupportZoneView {
  return { id, code, kind: "support", low, high, confidence: "高", spread: 0.01 };
}

beforeEach(() => {
  mockedInvoke.mockReset();
});

describe("标注存储", () => {
  it("换股票时取回该股票的标注", async () => {
    stubInvoke({ support_zones_get: [zone("b1", "700100")] });
    await loadZones("700100");
    expect(getZones().code).toBe("700100");
    expect(getZones().zones.map((item) => item.id)).toEqual(["b1"]);
  });

  it("保存已有标注时就地替换，不重复追加", async () => {
    stubInvoke({
      support_zones_get: [zone("b1", "700200"), zone("b2", "700200")],
      support_zone_save: zone("b1", "700200", 9.0, 10.0),
    });
    await loadZones("700200");
    await saveZone({ id: "b1", code: "700200", kind: "support", low: 9.0, high: 10.0 });

    const zones = getZones().zones;
    expect(zones).toHaveLength(2);
    expect(zones[0].id).toBe("b1");
    expect(zones[0].low).toBe(9.0);
    expect(zones[1].id).toBe("b2");
  });

  it("删除标注后从列表里移除", async () => {
    stubInvoke({
      support_zones_get: [zone("b1", "700300"), zone("b2", "700300")],
      support_zone_delete: undefined,
    });
    await loadZones("700300");
    await removeZone("b1");
    expect(getZones().zones.map((item) => item.id)).toEqual(["b2"]);
  });

  it("取标注失败时把错误留在快照里", async () => {
    stubInvoke(); // 没有登记 support_zones_get，调用直接失败
    await loadZones("700400");
    expect(getZones().code).toBe("700400");
    expect(getZones().zones).toEqual([]);
    expect(getZones().error).not.toBeNull();
  });
});
