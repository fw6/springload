import { HashRouter, NavLink, Route, Routes } from "react-router-dom";
import { toMessage } from "./api/invoke";
import { BeginnerModeProvider } from "./copy/BeginnerModeContext";
import { GlossaryProvider } from "./copy/GlossaryContext";
import { upDownLabel } from "./chart/palette";
import { MARKET_LABELS, MarketProvider, useMarket } from "./copy/MarketContext";
import { tradingRule } from "./copy/riskNotes";
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

/**
 * 市场切换：两个格子，当前那个用反白的油墨底。
 *
 * 它和版次条并排但不同轴：版次条是这一版看哪一步，这里是这一版看哪个市场。用油墨
 * 底而不是套红，是为了不跟版次条的套红抢眼。
 */
function MarketSwitch() {
  const { market, loaded, error, setMarket } = useMarket();

  return (
    <div className="flex flex-none items-stretch gap-1">
      <span className="flex items-center pr-1 text-micro text-ink-soft">市场</span>
      <div
        role="group"
        aria-label="交易市场"
        className="flex items-stretch gap-px border border-rule-strong bg-rule-strong"
      >
        {MARKET_LABELS.map((item) => {
          const active = market === item.market;
          return (
            <button
              key={item.market}
              type="button"
              data-slot="market-chip"
              aria-pressed={active}
              disabled={!loaded}
              onClick={() => {
                if (active || !loaded) return;
                void setMarket(item.market);
              }}
              className={cn(
                "px-3 py-1.5 text-body transition-colors disabled:opacity-45",
                active
                  ? "bg-ink font-medium text-paper"
                  : "bg-paper text-ink-soft hover:bg-slip hover:text-ink",
              )}
            >
              {item.label}
            </button>
          );
        })}
      </div>
      {error !== null ? (
        <span className="flex items-center pl-2 text-micro text-alert">
          切换失败：{toMessage(error)}
        </span>
      ) : null}
    </div>
  );
}

/** 报头：刊名在左，刊期与运行信息在右，下面压一条粗加细的双线。 */
function Masthead() {
  const { info } = useMarket();

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
        <span className="font-display text-note text-ink-soft">回调买入法工作台</span>
      </div>

      {/* 报头与版次条之间压一条粗加细的双线。 */}
      <div aria-hidden className="border-t-2 border-ink" />
      <div aria-hidden className="mt-[3px] border-t border-ink" />

      {/*
        版次条：左边五个版次，右边市场切换。版次条的底色就是格子之间的缝，
        nav 只占五个格子那么宽，底色才不会从最后一格一路铺到右端。
      */}
      <div className="flex flex-wrap items-stretch justify-between gap-x-4 gap-y-1.5">
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
        <MarketSwitch />
      </div>
    </header>
  );
}

/** 报尾：全刊统一的口径与风险说明，按市场换那一条交易制度的提示。 */
function Colophon() {
  const { market, info } = useMarket();
  // 市场名由后端给，界面上不另写一份。
  const label = info?.marketLabel ?? (market === "us" ? "美股" : "A 股");

  return (
    <footer className="flex flex-none flex-wrap items-center gap-x-3 border-t border-rule bg-paper px-4 py-1.5 text-micro text-ink-soft">
      <span>本刊所有数字为理论值，未计入交易成本与滑点。</span>
      <span aria-label={`${label}交易制度`}>{tradingRule(market)}</span>
      <span>{upDownLabel(market)}</span>
    </footer>
  );
}

/**
 * 外壳与五个版次。测试里用 memory router 包住它即可。
 *
 * 路由子树按市场换 key：切换市场后整棵子树重新挂载，各页按新市场重新取数，
 * 不用逐个页面写刷新逻辑。
 */
export function AppRoutes() {
  const { market } = useMarket();

  return (
    <div className="flex h-full min-h-0 flex-col bg-paper">
      <Masthead />
      <div className="min-h-0 flex-1" key={market}>
        <GlossaryProvider>
          <Routes>
            <Route path="/" element={<FlowPage />} />
            <Route path="/plan" element={<PlanPage />} />
            <Route path="/positions" element={<PositionsPage />} />
            <Route path="/journal" element={<JournalPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </GlossaryProvider>
      </div>
      <Colophon />
    </div>
  );
}

export default function App() {
  return (
    <HashRouter>
      <MarketProvider>
        <BeginnerModeProvider>
          <AppRoutes />
        </BeginnerModeProvider>
      </MarketProvider>
    </HashRouter>
  );
}
