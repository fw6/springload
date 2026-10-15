/**
 * 市场上下文：当前是 A 股还是美股，以及切换市场的动作。
 *
 * 切换走 `market_set` 命令：后端把工作区换到另一个市场的库与行情源，并把选择写进
 * 配置。前端的 `AppRoutes` 按市场给路由子树换一个 key，整棵树重新挂载，各页按新
 * 市场重新取数，所以切完看到的整站都是新市场的东西。
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { getAppInfo, type AppInfo, type Market } from "../api/app";
import { invoke, toMessage } from "../api/invoke";
import { resetZones } from "../chart/zoneStore";

/** 市场上下文的对外形状。 */
export interface MarketValue {
  /** 当前市场。读到配置前按 A 股处理。 */
  market: Market;
  /** `app_info` 的结果；还没取到时是 null。 */
  info: AppInfo | null;
  /** 是否已取到 `app_info`。 */
  loaded: boolean;
  /** 读取或切换失败的原因；成功时是 null。 */
  error: string | null;
  /** 切换市场并重新取一次应用信息。 */
  setMarket: (market: Market) => Promise<void>;
}

/** 两个市场在界面上的名字，切换控件用。 */
export const MARKET_LABELS: Array<{ market: Market; label: string }> = [
  { market: "cn", label: "A 股" },
  { market: "us", label: "美股" },
];

const DEFAULT_VALUE: MarketValue = {
  market: "cn",
  info: null,
  loaded: false,
  error: null,
  setMarket: async () => {},
};

const MarketContext = createContext<MarketValue>(DEFAULT_VALUE);

/** 读当前市场。没有 Provider 时按 A 股处理。 */
export function useMarket(): MarketValue {
  return useContext(MarketContext);
}

/** 当前市场的计价货币名。取不到应用信息时按市场兜底。 */
export function useCurrency(): string {
  const { market, info } = useMarket();
  return info?.currency ?? (market === "us" ? "美元" : "元");
}

/** 当前市场是否当日买入次日才能卖出。取不到应用信息时按市场兜底。 */
export function useTPlusOne(): boolean {
  const { market, info } = useMarket();
  return info?.tPlusOne ?? market === "cn";
}

/** 启动时取一次 `app_info`，之后由切换动作更新。 */
export function MarketProvider({ children }: { children: ReactNode }) {
  const [value, setValue] = useState<Omit<MarketValue, "setMarket">>({
    ...DEFAULT_VALUE,
  });

  useEffect(() => {
    let alive = true;
    getAppInfo()
      .then((info) => {
        if (!alive) return;
        setValue({ market: info.market, info, loaded: true, error: null });
      })
      .catch((cause: unknown) => {
        if (alive) setValue({ ...DEFAULT_VALUE, loaded: true, error: toMessage(cause) });
      });
    return () => {
      alive = false;
    };
  }, []);

  const setMarket = useCallback(async (target: Market) => {
    try {
      const next = await invoke<{ market: Market }>("market_set", { market: target });
      // 标注是模块级的外部存储，换市场时清掉，免得新市场看到上一个市场的标注。
      resetZones();
      const info = await getAppInfo();
      setValue({ market: next.market, info, loaded: true, error: null });
    } catch (cause: unknown) {
      // 切换失败时留在原市场，把原因交给切换控件显示，不静默吞掉。
      setValue((current) => ({ ...current, error: toMessage(cause) }));
    }
  }, []);

  const current = useMemo(() => ({ ...value, setMarket }), [value, setMarket]);

  return <MarketContext value={current}>{children}</MarketContext>;
}
