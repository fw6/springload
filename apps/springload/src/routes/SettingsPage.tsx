import { useEffect, useState, useTransition } from "react";
import type { Market } from "../api/app";
import {
  getSettings,
  importBarsCsv,
  updateSettings,
  type ImportReport,
  type MarketSettings,
  type Settings,
  type SettingsPatch,
} from "../api/settings";
import { AdjustmentPanel } from "../components/AdjustmentPanel";
import { Panel, SectionHead } from "../components/Panel";
import { SyncPanel } from "../components/SyncPanel";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../components/ui/alert";
import { Button, StatefulButton } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
import { RiskNotesDialog, useBeginnerMode } from "../copy/BeginnerModeContext";
import { useMarket } from "../copy/MarketContext";
import { feeItems, tradingRule } from "../copy/riskNotes";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** 小数比例显示成百分数，去掉浮点尾差。 */
function toPercent(value: number): number {
  return Number((value * 100).toFixed(6));
}

/**
 * 成交额阈值的显示单位：A 股习惯用亿元，美股用百万美元。
 *
 * 两个市场的成交额量级差一个数量级，同一个输入框换个单位才不至于让人数一长串零。
 */
const LIQUIDITY_UNIT = {
  cn: { label: "亿元", scale: 1e8 },
  us: { label: "百万美元", scale: 1e6 },
} as const;

/** 成交额按当前市场的单位显示，去掉浮点尾差。 */
function toLiquidityUnit(value: number, market: "cn" | "us"): number {
  return Number((value / LIQUIDITY_UNIT[market].scale).toFixed(6));
}

/** 本页直接编辑的字段；小白模式由 context 立即写回，不在这里保存。
 * 成交额下限与佣金费率随市场分开存，改的是当前市场那一份。 */
function pagePatch(settings: Settings, market: Market): SettingsPatch {
  const current: MarketSettings = market === "us" ? settings.us : settings.cn;
  return {
    riskPct: settings.riskPct,
    stopDistanceWarn: settings.stopDistanceWarn,
    chg6mThreshold: settings.chg6mThreshold,
    rankBand: settings.rankBand,
    convergenceThreshold: settings.convergenceThreshold,
    autostart: settings.autostart,
    feeModelEnabled: settings.feeModelEnabled,
    [market]: current,
  } as SettingsPatch;
}

/** 设置页：风险提示入口、筛选参数、数据源与同步面板、CSV 导入、小白模式开关。 */
export default function SettingsPage() {
  const { beginnerMode, setBeginnerMode } = useBeginnerMode();
  const { market, info } = useMarket();
  const unit = LIQUIDITY_UNIT[market];

  const [saved, setSaved] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, startSaving] = useTransition();

  /** 当前市场那一份参数：成交额下限与佣金费率。 */
  const current: MarketSettings | null =
    draft === null ? null : market === "us" ? draft.us : draft.cn;

  const [csvPath, setCsvPath] = useState("");
  const [importing, startImporting] = useTransition();
  const [importReport, setImportReport] = useState<ImportReport | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const [showRisk, setShowRisk] = useState(false);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((settings) => {
        if (!alive) return;
        setSaved(settings);
        setDraft(settings);
      })
      .catch((err: unknown) => {
        if (alive) setError(message(err));
      });
    return () => {
      alive = false;
    };
  }, []);

  if (error !== null && draft === null) {
    return (
      <div className="h-full overflow-auto p-4">
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>设置读取失败：{error}</AlertDescription>
          </AlertContent>
        </Alert>
      </div>
    );
  }

  if (draft === null || saved === null) {
    return (
      <div className="h-full overflow-auto p-4">
        <p className="text-note text-ink-soft">正在读取设置…</p>
      </div>
    );
  }

  const dirty =
    JSON.stringify(pagePatch(draft, market)) !== JSON.stringify(pagePatch(saved, market));

  const patch = (changes: Partial<Settings>) => setDraft({ ...draft, ...changes });

  /** 改当前市场那一份参数。 */
  const patchMarket = (changes: Partial<MarketSettings>) => {
    const key = market === "us" ? "us" : "cn";
    patch({ [key]: { ...draft[key], ...changes } } as Partial<Settings>);
  };

  const save = () => {
    startSaving(async () => {
      try {
        const next = await updateSettings(pagePatch(draft, market));
        setSaved(next);
        setDraft(next);
        setError(null);
      } catch (err: unknown) {
        setError(message(err));
      }
    });
  };

  const runImport = () => {
    startImporting(async () => {
      setImportError(null);
      setImportReport(null);
      try {
        setImportReport(await importBarsCsv(csvPath));
      } catch (err: unknown) {
        setImportError(message(err));
      }
    });
  };

  return (
    <div className="flex h-full flex-col gap-3.5 overflow-auto p-4">
      <Panel
        title="设置"
        className="max-w-[48rem]"
        action={
          <>
            {dirty ? <span className="text-note text-ink-soft">有未保存的改动</span> : null}
            <Button size="sm" variant="secondary" onClick={() => setShowRisk(true)}>
              查看风险提示
            </Button>
          </>
        }
      >
        <div className="flex flex-wrap items-center gap-x-8 gap-y-3 border-b border-rule pb-3">
          <Switch
            checked={beginnerMode}
            onCheckedChange={(next) => {
              void setBeginnerMode(next);
            }}
            label="小白模式（术语弹窗解释）"
          />
          <Switch
            checked={draft.autostart}
            onCheckedChange={(next) => patch({ autostart: next })}
            label="登录时自动启动"
          />
          <Switch
            checked={draft.feeModelEnabled}
            onCheckedChange={(next) => patch({ feeModelEnabled: next })}
            label="费用模型（交易日志逐笔扣减）"
          />
        </div>

        <SectionHead>筛选与风险参数</SectionHead>
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <Input
            label="单笔风险比例（%）"
            type="number"
            step="0.1"
            value={String(toPercent(draft.riskPct))}
            onChange={(value) => patch({ riskPct: Number(value) / 100 })}
          />
          <Input
            label="止损距离阈值（%）"
            type="number"
            step="0.1"
            value={String(toPercent(draft.stopDistanceWarn))}
            onChange={(value) => patch({ stopDistanceWarn: Number(value) / 100 })}
          />
          <Input
            label={`流动性阈值（${unit.label}）`}
            type="number"
            step="0.1"
            value={String(toLiquidityUnit(current?.liquidityThreshold ?? 0, market))}
            onChange={(value) => patchMarket({ liquidityThreshold: Number(value) * unit.scale })}
          />
          <Input
            label="近 6 个月涨幅阈值（%）"
            type="number"
            step="1"
            value={String(toPercent(draft.chg6mThreshold))}
            onChange={(value) => patch({ chg6mThreshold: Number(value) / 100 })}
          />
          <Input
            label="排名分位下限（%）"
            type="number"
            step="1"
            value={String(toPercent(draft.rankBand[0]))}
            onChange={(value) => patch({ rankBand: [Number(value) / 100, draft.rankBand[1]] })}
          />
          <Input
            label="排名分位上限（%）"
            type="number"
            step="1"
            value={String(toPercent(draft.rankBand[1]))}
            onChange={(value) => patch({ rankBand: [draft.rankBand[0], Number(value) / 100] })}
          />
          <Input
            label="近 20 日极差阈值（%）"
            type="number"
            step="1"
            value={String(toPercent(draft.convergenceThreshold))}
            onChange={(value) => patch({ convergenceThreshold: Number(value) / 100 })}
          />
          <Input
            label={
              market === "us" ? "佣金费率（%）" : "佣金费率（%，含单笔最低 5 元）"
            }
            type="number"
            step="0.001"
            min="0"
            value={String(toPercent(current?.commissionRate ?? 0))}
            onChange={(value) => patchMarket({ commissionRate: Number(value) / 100 })}
          />
        </div>
        <p className="text-note text-ink-soft">
          费用模型默认关闭。开启后按{feeItems(market)}逐笔扣减
          {market === "us" ? "，没有印花税与各项规费，也没有最低佣金。" : "。"}
        </p>

        <SectionHead>数据源与回补</SectionHead>
        <Input label="数据源" value={info?.dataSource ?? "未取到"} readOnly />

        <SyncPanel />

        <AdjustmentPanel />

        <div className="flex flex-col gap-2 border-t border-rule pt-3">
          <div className="flex items-end gap-3">
            <Input
              className="flex-1"
              label="CSV 文件路径"
              placeholder="~/bars.csv"
              value={csvPath}
              onChange={setCsvPath}
            />
            <StatefulButton
              size="sm"
              variant="secondary"
              state={importing ? "loading" : "idle"}
              loadingText="导入中…"
              disabled={csvPath.trim() === ""}
              onClick={() => runImport()}
            >
              导入
            </StatefulButton>
          </div>
          <span className="text-note text-ink-soft">
            CSV 列顺序不限，必需列为 code,date,open,high,low,close,volume,amount。
          </span>
        </div>

        {importError !== null ? (
          <Alert variant="destructive">
            <AlertIcon />
            <AlertContent>
              <AlertDescription>导入失败：{importError}</AlertDescription>
            </AlertContent>
          </Alert>
        ) : null}
        {importReport !== null ? (
          <Alert variant="info">
            <AlertContent>
              <AlertDescription>
                <p>
                  导入完成：写入 {importReport.rows} 行，跳过 {importReport.skipped} 行。
                </p>
                {importReport.errors.length > 0 ? (
                  <ul className="mt-1 flex flex-col gap-0.5 pl-4">
                    {importReport.errors.map((item) => (
                      <li key={item} className="list-disc">
                        {item}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </AlertDescription>
            </AlertContent>
          </Alert>
        ) : null}

        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>
              仓位计算结果为理论值，未计入交易成本与滑点；{tradingRule(market)}
            </AlertDescription>
          </AlertContent>
        </Alert>

        <div className="flex items-center gap-3">
          <StatefulButton
            size="sm"
            state={saving ? "loading" : "idle"}
            loadingText="保存中…"
            disabled={!dirty}
            onClick={() => save()}
          >
            保存
          </StatefulButton>
          {error !== null ? (
            <span className="text-note text-alert">保存失败：{error}</span>
          ) : null}
        </div>
      </Panel>

      <RiskNotesDialog
        open={showRisk}
        confirmLabel="关闭"
        onConfirm={() => setShowRisk(false)}
      />
    </div>
  );
}
