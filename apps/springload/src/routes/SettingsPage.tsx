import { useEffect, useState } from "react";
import { getAppInfo } from "../api/app";
import {
  getSettings,
  importBarsCsv,
  updateSettings,
  type ImportReport,
  type Settings,
  type SettingsPatch,
} from "../api/settings";
import { RiskNotesDialog, useBeginnerMode } from "../copy/BeginnerModeContext";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** 小数比例显示成百分数，去掉浮点尾差。 */
function toPercent(value: number): number {
  return Number((value * 100).toFixed(6));
}

/** 成交额（元）显示成亿元，去掉浮点尾差。 */
function toYi(value: number): number {
  return Number((value / 1e8).toFixed(6));
}

/** 本页直接编辑的字段；小白模式由 context 立即写回，不在这里保存。 */
function pagePatch(settings: Settings): SettingsPatch {
  return {
    riskPct: settings.riskPct,
    stopDistanceWarn: settings.stopDistanceWarn,
    liquidityThreshold: settings.liquidityThreshold,
    chg6mThreshold: settings.chg6mThreshold,
    rankBand: settings.rankBand,
    convergenceThreshold: settings.convergenceThreshold,
    autostart: settings.autostart,
  };
}

/** 设置页：风险提示入口、筛选参数、数据源与同步时间、CSV 导入、小白模式开关。 */
export default function SettingsPage() {
  const { beginnerMode, setBeginnerMode } = useBeginnerMode();

  const [saved, setSaved] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [dataSource, setDataSource] = useState<string | null>(null);
  const [lastSyncAt, setLastSyncAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const [csvPath, setCsvPath] = useState("");
  const [importing, setImporting] = useState(false);
  const [importReport, setImportReport] = useState<ImportReport | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const [showRisk, setShowRisk] = useState(false);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        setSaved(settings);
        setDraft(settings);
      })
      .catch((err: unknown) => setError(message(err)));
    getAppInfo()
      .then((info) => {
        setDataSource(info.dataSource);
        setLastSyncAt(info.lastSyncAt);
      })
      .catch(() => {
        setDataSource(null);
        setLastSyncAt(null);
      });
  }, []);

  if (error !== null && draft === null) {
    return (
      <div className="page">
        <div className="notice" data-tone="risk">设置读取失败：{error}</div>
      </div>
    );
  }

  if (draft === null || saved === null) {
    return (
      <div className="page">
        <p className="hint">正在读取设置…</p>
      </div>
    );
  }

  const dirty = JSON.stringify(pagePatch(draft)) !== JSON.stringify(pagePatch(saved));

  const patch = (changes: Partial<Settings>) => setDraft({ ...draft, ...changes });

  const save = async () => {
    setSaving(true);
    try {
      const next = await updateSettings(pagePatch(draft));
      setSaved(next);
      setDraft(next);
      setError(null);
    } catch (err: unknown) {
      setError(message(err));
    } finally {
      setSaving(false);
    }
  };

  const runImport = async () => {
    setImporting(true);
    setImportError(null);
    setImportReport(null);
    try {
      const report = await importBarsCsv(csvPath);
      setImportReport(report);
    } catch (err: unknown) {
      setImportError(message(err));
    } finally {
      setImporting(false);
    }
  };

  const confirmRisk = () => setShowRisk(false);

  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 680 }}>
        <div className="panel-head">
          <h2>设置</h2>
          <span className="toolbar-spacer" />
          <button type="button" className="ghost" onClick={() => setShowRisk(true)}>
            查看风险提示
          </button>
          {dirty ? <span className="hint">有未保存的改动</span> : null}
        </div>
        <div className="panel-body">
          <div className="field-row">
            <div className="field">
              <label htmlFor="beginner-mode">小白模式（术语弹窗解释）</label>
              <select
                id="beginner-mode"
                value={beginnerMode ? "on" : "off"}
                onChange={(event) => {
                  void setBeginnerMode(event.target.value === "on");
                }}
              >
                <option value="on">开启</option>
                <option value="off">关闭</option>
              </select>
            </div>
            <div className="field">
              <label htmlFor="autostart">登录时自动启动</label>
              <select
                id="autostart"
                value={draft.autostart ? "on" : "off"}
                onChange={(event) => patch({ autostart: event.target.value === "on" })}
              >
                <option value="on">开启</option>
                <option value="off">关闭</option>
              </select>
            </div>
          </div>

          <div className="field-row">
            <div className="field">
              <label htmlFor="risk-pct">单笔风险比例（%）</label>
              <input
                id="risk-pct"
                type="number"
                step="0.1"
                value={toPercent(draft.riskPct)}
                onChange={(event) => patch({ riskPct: Number(event.target.value) / 100 })}
              />
            </div>
            <div className="field">
              <label htmlFor="stop-distance">止损距离阈值（%）</label>
              <input
                id="stop-distance"
                type="number"
                step="0.1"
                value={toPercent(draft.stopDistanceWarn)}
                onChange={(event) => patch({ stopDistanceWarn: Number(event.target.value) / 100 })}
              />
            </div>
          </div>

          <div className="field-row">
            <div className="field">
              <label htmlFor="liquidity">流动性阈值（亿元）</label>
              <input
                id="liquidity"
                type="number"
                step="0.1"
                value={toYi(draft.liquidityThreshold)}
                onChange={(event) =>
                  patch({ liquidityThreshold: Number(event.target.value) * 1e8 })
                }
              />
            </div>
            <div className="field">
              <label htmlFor="chg6m">近 6 个月涨幅阈值（%）</label>
              <input
                id="chg6m"
                type="number"
                step="1"
                value={toPercent(draft.chg6mThreshold)}
                onChange={(event) => patch({ chg6mThreshold: Number(event.target.value) / 100 })}
              />
            </div>
          </div>

          <div className="field-row">
            <div className="field">
              <label htmlFor="rank-low">排名分位下限（%）</label>
              <input
                id="rank-low"
                type="number"
                step="1"
                value={toPercent(draft.rankBand[0])}
                onChange={(event) =>
                  patch({ rankBand: [Number(event.target.value) / 100, draft.rankBand[1]] })
                }
              />
            </div>
            <div className="field">
              <label htmlFor="rank-high">排名分位上限（%）</label>
              <input
                id="rank-high"
                type="number"
                step="1"
                value={toPercent(draft.rankBand[1])}
                onChange={(event) =>
                  patch({ rankBand: [draft.rankBand[0], Number(event.target.value) / 100] })
                }
              />
            </div>
          </div>

          <div className="field">
            <label htmlFor="convergence">近 20 日极差阈值（%）</label>
            <input
              id="convergence"
              type="number"
              step="1"
              value={toPercent(draft.convergenceThreshold)}
              onChange={(event) =>
                patch({ convergenceThreshold: Number(event.target.value) / 100 })
              }
            />
          </div>

          <div className="field-row">
            <div className="field">
              <label htmlFor="data-source">数据源</label>
              <input id="data-source" value={dataSource ?? "未取到"} readOnly />
            </div>
            <div className="field">
              <label htmlFor="last-sync">最近一次同步时间</label>
              <input id="last-sync" value={lastSyncAt ?? "尚未同步"} readOnly />
            </div>
          </div>

          <div className="field">
            <label htmlFor="csv-path">CSV 文件路径</label>
            <div className="field-row">
              <input
                id="csv-path"
                value={csvPath}
                placeholder="~/bars.csv"
                onChange={(event) => setCsvPath(event.target.value)}
              />
              <button
                type="button"
                className="ghost"
                onClick={runImport}
                disabled={importing || csvPath.trim() === ""}
              >
                {importing ? "导入中…" : "导入"}
              </button>
            </div>
            <span className="hint">
              CSV 列顺序不限，必需列为 code,date,open,high,low,close,volume,amount。
            </span>
          </div>

          {importError !== null ? (
            <div className="notice" data-tone="risk">导入失败：{importError}</div>
          ) : null}
          {importReport !== null ? (
            <div className="notice" data-tone="info">
              <p>
                导入完成：写入 {importReport.rows} 行，跳过 {importReport.skipped} 行。
              </p>
              {importReport.errors.length > 0 ? (
                <ul>
                  {importReport.errors.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}

          <div className="notice" data-tone="risk">
            仓位计算结果为理论值，未计入交易成本与滑点；A 股 T+1 制度下当日买入无法当日卖出。
          </div>

          <div className="panel-foot" style={{ borderTop: "none", padding: 0 }}>
            <button type="button" className="primary" onClick={save} disabled={!dirty || saving}>
              {saving ? "保存中…" : "保存"}
            </button>
            {error !== null ? <span className="hint">保存失败：{error}</span> : null}
          </div>
        </div>
      </div>

      {showRisk ? (
        <RiskNotesDialog confirmLabel="关闭" onConfirm={confirmRisk} />
      ) : null}
    </div>
  );
}
