import { useEffect, useMemo, useRef, useState, useTransition } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { toMessage } from "../../api/invoke";
import {
  screenLast,
  screenStocks,
  watchlistAdd,
  type ScreenReport,
  type ScreenRow,
} from "../../api/screen";
import { AiCard } from "../../components/AiCard";
import { DataTable, SortButton, Td, Th, Tr } from "../../components/DataTable";
import { Flag } from "../../components/Flag";
import { TermHint } from "../../components/TermHint";
import { TOOLTIP_TEXT_CLASS, Tooltip } from "../../components/Tooltip";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../../components/ui/alert";
import { StatefulButton } from "../../components/ui/button";
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

/**
 * 未入选表的行高估值。真实高度由 `measureElement` 逐行量出来，
 * 这个数字只用来决定第一屏要挂多少行。
 */
const REJECTED_ROW_ESTIMATE = 33;

/** 未入选表一次挂到视口外面的行数：滚动时留出缓冲，不会看到空白。 */
const REJECTED_OVERSCAN = 12;

/**
 * 未入选表视口的高度上限，大约露出十二行。
 * 窗口矮的时候按窗口高的一半收窄，免得表格把面板挤得只剩它自己。
 */
const REJECTED_VIEWPORT_MAX = 400;

function rejectedViewportHeight(): number {
  return Math.round(Math.min(REJECTED_VIEWPORT_MAX, window.innerHeight * 0.5));
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

/** 四条判定的表头，带悬停解释。 */
function FlagHeader({ label, tooltip }: { label: string; tooltip: string }) {
  return (
    <Th align="center">
      <Tooltip text={tooltip}>
        <span className={TOOLTIP_TEXT_CLASS}>{label}</span>
      </Tooltip>
    </Th>
  );
}

/** 小标题：栏线下面一行加粗的宋体。 */
function SectionHead({ children }: { children: string }) {
  return (
    <h3 className="border-b border-rule pb-1 font-display text-body font-bold text-ink">
      {children}
    </h3>
  );
}

/**
 * 第 1 版：跑筛选、看入选与未入选两张表、把股票加进观察名单。
 *
 * 数据由本步自己的 hook 取（筛选结果），观察名单走 `flow` 写回。
 */
export default function Step1({ flow }: StepProps) {
  const [report, setReport] = useState<ScreenReport | null>(null);
  const [busy, startBusy] = useTransition();
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

  /**
   * 未入选动辄三千多只，一次全挂上去会把主线程占住几百毫秒，
   * 切换版次时能明显看到卡一下。这里只渲染视口附近的那些行，
   * 上下各垫一条与剩余行等高、不画线的空行把滚动条撑到正确的长度。
   */
  const rejectedViewport = useRef<HTMLDivElement | null>(null);
  const rejectedRows = useVirtualizer({
    count: rejected.length,
    getScrollElement: () => rejectedViewport.current,
    estimateSize: () => REJECTED_ROW_ESTIMATE,
    getItemKey: (index) => rejected[index]?.code ?? index,
    overscan: REJECTED_OVERSCAN,
  });
  const rejectedItems = rejectedRows.getVirtualItems();
  const rejectedTotal = rejectedRows.getTotalSize();
  const rejectedFirst = rejectedItems[0];
  const rejectedLast = rejectedItems[rejectedItems.length - 1];
  const [rejectedHeight, setRejectedHeight] = useState(rejectedViewportHeight);
  useEffect(() => {
    const update = () => setRejectedHeight(rejectedViewportHeight());
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  const runScreen = () => {
    startBusy(async () => {
      try {
        setReport(await screenStocks());
        await flow.reloadWatchlist();
        setError(null);
      } catch (failure: unknown) {
        setError(toMessage(failure));
      }
    });
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
   * 但本步要留在第 1 版继续挑股票，所以把版次改回来。
   */
  const selectRow = (code: string) => {
    flow.selectStock(code);
    flow.setStep(1);
  };

  const selected = flow.selected;
  const alreadyIn = selected !== null && flow.watchlist.some((row) => row.code === selected);

  const sortProps = (key: SortKey, label: string, tooltip: string) => ({
    label,
    tooltip,
    active: sort.key === key,
    desc: sort.desc,
    onClick: () => onSort(key),
  });

  return (
    <>
      <p className="text-body leading-relaxed text-ink-soft">
        这一版只做一件事：找出正在休息的强势股。四条判定全中才进观察名单 ——{" "}
        <TermHint termKey="pullback" />。
      </p>

      <div className="flex flex-wrap items-center gap-3">
        <StatefulButton
          size="sm"
          state={busy ? "loading" : "idle"}
          loadingText="筛选进行中…"
          onClick={() => runScreen()}
        >
          开始筛选
        </StatefulButton>
        <span className="text-note text-ink-soft">
          {report === null
            ? "还没有跑过筛选。筛选参数在设置页调整。"
            : `跑在 ${report.universe} 只股票上，用了 ${report.elapsedMs} 毫秒。`}
        </span>
      </div>

      {error !== null ? (
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>筛选失败：{error}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      <SectionHead>{`入选（${picked.length} 只）`}</SectionHead>
      <DataTable>
        <thead>
          <tr>
            <Th>代码</Th>
            <Th>名称</Th>
            <Th>板块</Th>
            <Th align="right">最新价</Th>
            <Th align="right">
              <SortButton {...sortProps("chg1m", "近 1 月", TOOLTIPS.chg1m)} />
            </Th>
            <Th align="right">
              <SortButton {...sortProps("chg6m", "近 6 月", TOOLTIPS.chg6m)} />
            </Th>
            <Th align="right">
              <SortButton {...sortProps("rankPct", "排名", TOOLTIPS.rankPct)} />
            </Th>
            <FlagHeader label="C1" tooltip={TOOLTIPS.c1} />
            <FlagHeader label="C2" tooltip={TOOLTIPS.c2} />
            <FlagHeader label="C3" tooltip={TOOLTIPS.c3} />
            <FlagHeader label="C4" tooltip={TOOLTIPS.c4} />
          </tr>
        </thead>
        <tbody>
          {picked.map((row) => (
            <Tr
              key={row.code}
              selected={flow.selected === row.code}
              onClick={() => selectRow(row.code)}
            >
              <Td>{row.code}</Td>
              <Td>{row.name}</Td>
              <Td className="text-ink-soft">{row.sector ?? "—"}</Td>
              <Td align="right">{price(row.price)}</Td>
              <Td align="right">{pct(row.chg1m)}</Td>
              <Td align="right">{pct(row.chg6m)}</Td>
              <Td align="right">{pct(row.rankPct)}</Td>
              <Td align="center">
                <Flag ok={row.c1} />
              </Td>
              <Td align="center">
                <Flag ok={row.c2} />
              </Td>
              <Td align="center">
                <Flag ok={row.c3} />
              </Td>
              <Td align="center">
                <Flag ok={row.c4} />
              </Td>
            </Tr>
          ))}
        </tbody>
      </DataTable>

      <SectionHead>{`未入选（${rejected.length} 只）`}</SectionHead>
      <DataTable
        ref={rejectedViewport}
        rowCount={rejected.length + 1}
        style={{ height: rejectedHeight }}
      >
        <thead>
          <tr>
            <Th>代码</Th>
            <Th>名称</Th>
            <Th align="right">近 6 月</Th>
            <Th align="right">排名</Th>
            <Th>未中的条件</Th>
          </tr>
        </thead>
        <tbody>
          {rejectedFirst !== undefined && rejectedFirst.start > 0 ? (
            <tr aria-hidden className="border-0">
              <td className="border-0 p-0" colSpan={5} style={{ height: rejectedFirst.start }} />
            </tr>
          ) : null}
          {rejectedItems.map((item) => {
            const row = rejected[item.index];
            if (row === undefined) return null;
            return (
              <Tr
                key={row.code}
                ref={rejectedRows.measureElement}
                data-index={item.index}
                // 行高给一个固定值：窗口化要按行高算每一行落在哪儿，
                // 内容偶尔换行时再靠 measureElement 逐行校正。
                style={{ height: REJECTED_ROW_ESTIMATE }}
                // 行号把表头那一行算进去，辅助技术读出来才是「第几行，共几行」。
                aria-rowindex={item.index + 2}
                selected={flow.selected === row.code}
                onClick={() => selectRow(row.code)}
              >
                <Td>{row.code}</Td>
                <Td>{row.name}</Td>
                <Td align="right">{pct(row.chg6m)}</Td>
                <Td align="right">{pct(row.rankPct)}</Td>
                <Td className="text-warn">{missedLabels(row).join(" · ")}</Td>
              </Tr>
            );
          })}
          {rejectedLast !== undefined && rejectedTotal - rejectedLast.end > 0 ? (
            <tr aria-hidden className="border-0">
              <td
                className="border-0 p-0"
                colSpan={5}
                style={{ height: rejectedTotal - rejectedLast.end }}
              />
            </tr>
          ) : null}
        </tbody>
      </DataTable>

      <div className="flex flex-wrap items-center gap-3">
        <StatefulButton
          size="sm"
          disabled={selected === null || alreadyIn}
          onClick={() => selected !== null && void addToWatchlist(selected)}
        >
          加入观察名单
        </StatefulButton>
        <span className="text-note text-ink-soft">已加入 {flow.watchlist.length} 只</span>
      </div>

      <AiCard code={flow.selected} scope={1} />
    </>
  );
}
