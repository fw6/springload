import { useEffect, useState } from "react";
import { HashRouter, NavLink, Route, Routes } from "react-router-dom";
import { getAppInfo, type AppInfo } from "./api/app";
import { GlossaryProvider } from "./copy/GlossaryContext";
import FlowPage from "./flow/FlowPage";
import JournalPage from "./routes/JournalPage";
import PlanPage from "./routes/PlanPage";
import PositionsPage from "./routes/PositionsPage";
import SettingsPage from "./routes/SettingsPage";

/** 五个路由与导航标签。路由表由本 ticket 一次写好，后续 ticket 只改各自的页面文件。 */
export const ROUTES = [
  { to: "/", label: "主流程" },
  { to: "/plan", label: "计划卡" },
  { to: "/positions", label: "持仓跟踪" },
  { to: "/journal", label: "交易日志" },
  { to: "/settings", label: "设置" },
];

/** 顶栏：品牌、版本号、数据源与最近同步时间。 */
function Topbar() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAppInfo()
      .then(setInfo)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }, []);

  return (
    <header className="topbar">
      <div className="brand">
        蓄势 Springload<small>回调买入法工作台</small>
      </div>
      <div className="topbar-right">
        {error !== null ? <span>应用信息读取失败：{error}</span> : null}
        {info !== null ? (
          <>
            <span>版本 {info.version}</span>
            <span>{info.dataSource}</span>
            <span>最近同步 {info.lastSyncAt ?? "尚未同步"}</span>
          </>
        ) : null}
      </div>
    </header>
  );
}

/** 外壳与五个路由。测试里用 memory router 包住它即可。 */
export function AppRoutes() {
  return (
    <GlossaryProvider>
      <div className="app">
        <Topbar />
        <nav className="tabs">
          {ROUTES.map((route) => (
            <NavLink key={route.to} to={route.to} end className="tab">
              {route.label}
            </NavLink>
          ))}
        </nav>
        <Routes>
          <Route path="/" element={<FlowPage />} />
          <Route path="/plan" element={<PlanPage />} />
          <Route path="/positions" element={<PositionsPage />} />
          <Route path="/journal" element={<JournalPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
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
