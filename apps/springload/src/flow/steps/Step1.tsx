import { useEffect, useMemo, useState } from "react";
import { toMessage } from "../../api/invoke";
import {
  screenLast,
  screenStocks,
  watchlistAdd,
  type ScreenReport,
  type ScreenRow,
} from "../../api/screen";
import { AiCard } from "../../components/AiCard";
import { Flag } from "../../components/Flag";
import { TermHint } from "../../components/TermHint";
import { Tooltip } from "../../components/Tooltip";
import type { StepProps } from "../useFlow";

/** 两张表共用的排序键。 */
type SortKey = "chg1m" | "chg6m" | "rankPct";

interface Sort {
  key: SortKey;
  /** true 表示从大到小。 */
  desc: boolean;
}

/** 列头的一句话解释。 */
const TOOLTIPS: Record<SortKey | "c1" | "c2" | "c3" | "c4", string> = {
  chg1m: "近 1 个月（21 个交易日）的涨幅。",
  chg6m: "近 6 个月（126 个交易日）的涨幅。",
  rankPct: "近 1 个月涨幅在全部候选股里的分位。最强的 20% 会被排除，落在前 20%~30% 这一段才算中 C1。",
  c1: "C1 涨幅条件：近 6 个月涨幅超过阈值，且近 1 个月涨幅的排名落在前 20%~30% 这一段。",
  c2: "C2 结构条件：当前回调低点高于上一次回调低点，且近 20 日极差低于阈值。",
  c3: "C3 均线条件：收盘价站在 9 日均线之上，且 EMA9 与 EMA21 走平或向上。",
  c4: "C4 排除条件：高点与低点均下降，且收盘价跌破向下的 EMA21。命中则不入选。",
};

/** 涨幅按百分数展示；没有数据时显示破折号。 */
function pct(value: number | null): string {
  return value === null ? "—" : `${(value * 100).toFixed(1)}%`;
}

/** 价格保留两位小数。 */
function price(value: number): string {
  return value.toFixed(2);
}

/** 未入选的股票差在哪几条上。 */
export function missedLabels(row: ScreenRow): string[] {
  const missed: string[] = [];
  if (!row.c1) missed.push("C1 涨幅");
  if (!row.c2) missed.push("C2 结构");
  if (!row.c3) missed.push("C3 均线");
  if (row.c4) missed.push("C4 排除命中");
  if (!row.liquid) missed.push("成交额不足");
  return missed;
}

/** 按排序键排一行；缺数据的排在最后，同值按代码排。 */
function sortRows(rows: ScreenRow[], sort: Sort): ScreenRow[] {
  return [...rows].sort((left, right) => {
    const a = left[sort.key];
    const b = right[sort.key];
    if (a === null && b === null) return left.code.localeCompare(right.code);
    if (a === null) return 1;
    if (b === null) return -1;
    if (a !== b) return sort.desc ? b - a : a - b;
    return left.code.localeCompare(right.code);
  });
}

/** 可点击排序的列头。 */
function SortHeader({
  label,
  tooltip,
  sortKey,
  sort,
  onSort,
}: {
  label: string;
  tooltip: string;
  sortKey: SortKey;
  sort: Sort;
  onSort: (key: SortKey) => void;
}) {
  const active = sort.key === sortKey;
  return (
    <th>
      <Tooltip text={tooltip}>
        <button
          type="button"
          className="ghost"
          style={{ border: "none", background: "transparent", padding: 0, color: "inherit" }}
          aria-label={`${label}排序`}
          onClick={() => onSort(sortKey)}
        >
          {label}
          {active ? (sort.desc ? " ↓" : " ↑") : ""}
        </button>
      </Tooltip>
    </th>
  );
}

/** 四条判定的表头，带悬停解释。 */
function FlagHeader({ label, tooltip }: { label: string; tooltip: string }) {
  return (
    <th>
      <Tooltip text={tooltip}>
        <span>{label}</span>
      </Tooltip>
    </th>
  );
}

/**
 * 第 1 步面板：跑筛选、看入选与未入选两张表、把股票加进观察名单。
 *
 * 数据由本步自己的 hook 取（筛选结果），观察名单走 `flow` 写回。
 */
export default function Step1({ flow }: StepProps) {
  const [report, setReport] = useState<ScreenReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sort, setSort] = useState<Sort>({ key: "rankPct", desc: false });

  useEffect(() => {
    let alive = true;
    screenLast()
      .then((last) => {
        if (alive) setReport(last);
      })
      .catch((failure: unknown) => {
        if (alive) setError(toMessage(failure));
      });
    return () => {
      alive = false;
    };
  }, []);

  const onSort = (key: SortKey) => {
    setSort((current) =>
      current.key === key ? { key, desc: !current.desc } : { key, desc: key !== "rankPct" },
    );
  };

  const picked = useMemo(
    () => (report === null ? [] : sortRows(report.picked, sort)),
    [report, sort],
  );
  const rejected = useMemo(
    () => (report === null ? [] : sortRows(report.rejected, sort)),
    [report, sort],
  );

  const runScreen = async () => {
    setBusy(true);
    setError(null);
    try {
      setReport(await screenStocks());
      await flow.reloadWatchlist();
    } catch (failure: unknown) {
      setError(toMessage(failure));
    } finally {
      setBusy(false);
    }
  };

  const addToWatchlist = async (code: string) => {
    setError(null);
    try {
      await watchlistAdd(code);
      await flow.reloadWatchlist();
    } catch (failure: unknown) {
      setError(toMessage(failure));
    }
  };

  /**
   * 选中一行。`selectStock` 会顺带让 Rust 重新判定解锁状态，
   * 但本步要留在第 1 步继续挑股票，所以把步改回来。
   */
  const selectRow = (code: string) => {
    flow.selectStock(code);
    flow.setStep(1);
  };

  const selected = flow.selected;
  const alreadyIn = selected !== null && flow.watchlist.some((row) => row.code === selected);

  return (
    <>
      <p className="hint">
        这一步只做一件事：找出正在休息的强势股。四条判定全中才进观察名单 ——{" "}
        <TermHint termKey="pullback" />。
      </p>

      <div className="field-row">
        <button type="button" className="primary" onClick={() => void runScreen()} disabled={busy}>
          {busy ? "筛选进行中…" : "开始筛选"}
        </button>
        <span className="hint">
          {report === null
            ? "还没有跑过筛选。筛选参数在设置页调整。"
            : `跑在 ${report.universe} 只股票上，用了 ${report.elapsedMs} 毫秒。`}
        </span>
      </div>

      {error !== null ? (
        <div className="notice" data-tone="risk">
          筛选失败：{error}
        </div>
      ) : null}

      <h3 className="block-title">入选（{picked.length} 只）</h3>
      <table className="grid">
        <thead>
          <tr>
            <th className="left">代码</th>
            <th className="left">名称</th>
            <th className="left">板块</th>
            <th>最新价</th>
            <SortHeader
              label="近 1 月"
              tooltip={TOOLTIPS.chg1m}
              sortKey="chg1m"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader
              label="近 6 月"
              tooltip={TOOLTIPS.chg6m}
              sortKey="chg6m"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader
              label="排名"
              tooltip={TOOLTIPS.rankPct}
              sortKey="rankPct"
              sort={sort}
              onSort={onSort}
            />
            <FlagHeader label="C1" tooltip={TOOLTIPS.c1} />
            <FlagHeader label="C2" tooltip={TOOLTIPS.c2} />
            <FlagHeader label="C3" tooltip={TOOLTIPS.c3} />
            <FlagHeader label="C4" tooltip={TOOLTIPS.c4} />
          </tr>
        </thead>
        <tbody>
          {picked.map((row) => (
            <tr
              key={row.code}
              data-selected={flow.selected === row.code}
              style={{ cursor: "pointer" }}
              onClick={() => selectRow(row.code)}
            >
              <td className="left mono">{row.code}</td>
              <td className="left">{row.name}</td>
              <td className="left" style={{ color: "#64748b" }}>
                {row.sector ?? "—"}
              </td>
              <td className="mono">{price(row.price)}</td>
              <td className="mono">{pct(row.chg1m)}</td>
              <td className="mono">{pct(row.chg6m)}</td>
              <td className="mono">{pct(row.rankPct)}</td>
              <td>
                <Flag ok={row.c1} />
              </td>
              <td>
                <Flag ok={row.c2} />
              </td>
              <td>
                <Flag ok={row.c3} />
              </td>
              <td>
                <Flag ok={row.c4} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3 className="block-title">未入选（{rejected.length} 只）</h3>
      <table className="grid">
        <thead>
          <tr>
            <th className="left">代码</th>
            <th className="left">名称</th>
            <th>近 6 月</th>
            <th>排名</th>
            <th className="left">未中的条件</th>
          </tr>
        </thead>
        <tbody>
          {rejected.map((row) => (
            <tr
              key={row.code}
              data-selected={flow.selected === row.code}
              style={{ cursor: "pointer" }}
              onClick={() => selectRow(row.code)}
            >
              <td className="left mono">{row.code}</td>
              <td className="left">{row.name}</td>
              <td className="mono">{pct(row.chg6m)}</td>
              <td className="mono">{pct(row.rankPct)}</td>
              <td className="left" style={{ color: "#b45309" }}>
                {missedLabels(row).join(" · ")}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <div className="panel-foot">
        <button
          type="button"
          className="primary"
          disabled={selected === null || alreadyIn}
          onClick={() => selected !== null && void addToWatchlist(selected)}
        >
          加入观察名单
        </button>
        <span className="hint">已加入 {flow.watchlist.length} 只</span>
      </div>

      <AiCard code={flow.selected} scope={1} />
    </>
  );
}
