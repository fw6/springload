import { useEffect, useState } from "react";
import { HashRouter, NavLink, Route, Routes } from "react-router-dom";
import { getAppInfo, type AppInfo } from "./api/app";
import { toMessage } from "./api/invoke";
import { GlossaryProvider } from "./copy/GlossaryContext";
import { isMacOS } from "./lib/platform";
import { cn } from "./lib/utils";
import FlowPage from "./flow/FlowPage";
import JournalPage from "./routes/JournalPage";
import PlanPage from "./routes/PlanPage";
import PositionsPage from "./routes/PositionsPage";
import SettingsPage from "./routes/SettingsPage";

/** 五个版次。 */
export const ROUTES = [
  { to: "/", label: "主流程" },
  { to: "/plan", label: "计划卡" },
  { to: "/positions", label: "持仓跟踪" },
  { to: "/journal", label: "交易日志" },
  { to: "/settings", label: "设置" },
];

const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];

/** 今天的刊期：2026年10月14日 星期三。 */
function today(): string {
  const now = new Date();
  return `${now.getFullYear()}年${now.getMonth() + 1}月${now.getDate()}日 星期${
    WEEKDAYS[now.getDay()]
  }`;
}

/** 报头：刊名在左，刊期与运行信息在右，下面压一条粗加细的双线。 */
function Masthead() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getAppInfo()
      .then((next) => {
        if (alive) setInfo(next);
      })
      .catch((cause: unknown) => {
        if (alive) setError(toMessage(cause));
      });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <header
      // 报头整条都能拖动窗口。`deep` 让子元素区域也生效，
      // 里面的链接与按钮会被 Tauri 自动排除，不影响点击。
      data-tauri-drag-region="deep"
      className="flex flex-none flex-col bg-paper px-4 select-none"
    >
      {/*
        报眉：macOS 上红黄绿三个按钮浮在这一条里，所以这条窄带取 macOS 标题栏的
        高度 32px——按钮居中在标题栏里，日期那行居中在这条窄带里，两者才落在同一
        条中线上。其余平台由系统画标题栏，这条只用来放刊期与运行信息，28px 够用。
        左侧让开 80px：这台机器上按钮占窗口内 x 8~70，系统换版本会挪，
        留宽一点才不会压到。
      */}
      <div
        data-slot="masthead-line"
        className={cn(
          "flex flex-none items-center gap-x-2 font-mono text-micro text-ink-soft",
          isMacOS() ? "h-8 pl-20" : "h-7",
        )}
      >
        <span>日刊 · {today()}</span>
        <span className="ml-auto flex flex-wrap items-center gap-x-2">
          {info !== null ? (
            <>
              <span>版本 {info.version}</span>
              <span aria-hidden>·</span>
              <span>{info.dataSource}</span>
              <span aria-hidden>·</span>
              <span>最近同步 {info.lastSyncAt ?? "尚未同步"}</span>
            </>
          ) : null}
          {error !== null ? (
            <span className="text-alert">应用信息读取失败：{error}</span>
          ) : null}
        </span>
      </div>

      {/* 报头行：左边是刊名与英文名，右边压一句刊物的定位。 */}
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 pt-3 pb-2.5">
        <span className="flex flex-wrap items-baseline gap-x-2.5">
          <span className="press font-display text-masthead font-bold tracking-tight text-ink">
            蓄势
          </span>
          <span className="font-mono text-micro tracking-[0.3em] text-ink-soft">SPRINGLOAD</span>
        </span>
        <span className="font-display text-note text-ink-soft">A 股回调买入法工作台</span>
      </div>

      {/* 报头与版次条之间压一条粗加细的双线。 */}
      <div aria-hidden className="border-t-2 border-ink" />
      <div aria-hidden className="mt-[3px] border-t border-ink" />

      {/*
        版次条的底色就是格子之间的缝：nav 只占五个格子那么宽，
        底色才不会从最后一格一路铺到右端。
      */}
      <nav aria-label="主导航" className="flex w-fit flex-wrap items-stretch gap-px bg-rule-strong">
        {ROUTES.map((route) => (
          <NavLink
            key={route.to}
            to={route.to}
            end
            className={({ isActive }) =>
              cn(
                "flex items-center px-3.5 py-1.5 text-body transition-colors",
                isActive
                  ? "bg-proof font-medium text-paper"
                  : "bg-paper text-ink-soft hover:bg-slip hover:text-ink",
              )
            }
          >
            {route.label}
          </NavLink>
        ))}
      </nav>
    </header>
  );
}

/** 报尾：全刊统一的口径与风险说明。 */
function Colophon() {
  return (
    <footer className="flex flex-none flex-wrap items-center gap-x-3 border-t border-rule bg-paper px-4 py-1.5 text-micro text-ink-soft">
      <span>本刊所有数字为理论值，未计入交易成本与滑点。</span>
      <span>A 股 T+1，当日买入次日才能卖出。</span>
      <span>红涨绿跌。</span>
    </footer>
  );
}

/** 外壳与五个版次。测试里用 memory router 包住它即可。 */
export function AppRoutes() {
  return (
    <GlossaryProvider>
      <div className="flex h-full min-h-0 flex-col bg-paper">
        <Masthead />
        <div className="min-h-0 flex-1">
          <Routes>
            <Route path="/" element={<FlowPage />} />
            <Route path="/plan" element={<PlanPage />} />
            <Route path="/positions" element={<PositionsPage />} />
            <Route path="/journal" element={<JournalPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </div>
        <Colophon />
      </div>
    </GlossaryProvider>
  );
}

export default function App() {
  return (
    <HashRouter>
      <AppRoutes />
    </HashRouter>
  );
}
