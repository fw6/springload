import { useCallback, useEffect, useState, useTransition } from "react";
import { toMessage } from "../api/invoke";
import {
  journalAddManual,
  journalExportCsv,
  journalGet,
  journalReview,
  type JournalView,
  type ManualEntry,
} from "../api/journal";
import { DataTable, Td, Th, Tr } from "../components/DataTable";
import { Panel, SectionHead, StatGrid, StatTile } from "../components/Panel";
import { TermHint } from "../components/TermHint";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../components/ui/alert";
import { Button, StatefulButton } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Stamp } from "../components/ui/stamp";
import { Typesetting } from "../components/ui/typesetting";
import { useBeginnerMode } from "../copy/BeginnerModeContext";
import { useCurrency, useMarket } from "../copy/MarketContext";
import { feeItems, riskNotes } from "../copy/riskNotes";

/** 统计口径的固定标注，与 Rust 侧一致。 */
export const STATS_NOTE = "未计入交易成本与滑点";

/** 价格保留两位小数。 */
function price(value: number | null): string {
  return value === null ? "—" : value.toFixed(2);
}

/** 比例按百分数展示，一位小数。 */
function pct(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

/** 空白的手动补录表单。 */
const BLANK: ManualEntry = {
  code: "",
  entryDate: "",
  entryPrice: 0,
  stopPrice: 0,
  exitPrice: null,
  exitDate: null,
  quantity: 100,
  note: null,
};

/** 交易日志：记录表、统计摘要、手动补录、CSV 导出与 AI 复盘。 */
export default function JournalPage() {
  const currency = useCurrency();
  const { market } = useMarket();
  const [view, setView] = useState<JournalView | null>(null);
  const [form, setForm] = useState<ManualEntry>(BLANK);
  const [filled, setFilled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, startBusy] = useTransition();
  const { beginnerMode } = useBeginnerMode();

  const load = useCallback(async () => {
    try {
      setView(await journalGet());
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const run = useCallback(
    (action: () => Promise<string>) => {
      startBusy(async () => {
        try {
          setStatus(await action());
          setError(null);
          await load();
        } catch (cause: unknown) {
          setError(toMessage(cause));
        }
      });
    },
    [load],
  );

  const submit = useCallback(() => {
    run(async () => {
      await journalAddManual(form);
      setForm(BLANK);
      setFilled(false);
      return "已补录一笔历史交易。";
    });
  }, [form, run]);

  const exportCsv = useCallback(() => {
    run(async () => {
      const path = `springload-journal-${new Date().toISOString().slice(0, 10)}.csv`;
      await journalExportCsv(path);
      return `已导出 CSV：${path}`;
    });
  }, [run]);

  const stats = view?.stats ?? null;
  const feeOn = view?.feeModelEnabled === true;

  return (
    <div className="flex h-full flex-col gap-3.5 overflow-auto p-4">
      <Panel
        title="交易日志"
        action={
          <>
            <span className="text-note text-ink-soft">
              {feeOn
                ? `费用模型已开启（佣金费率 ${pct(view?.commissionRate ?? 0)}）`
                : "费用模型关闭"}
            </span>
            <Button size="sm" variant="secondary" onClick={() => exportCsv()} disabled={busy}>
              导出 CSV
            </Button>
          </>
        }
      >
        {error !== null ? (
          <Alert variant="destructive">
            <AlertIcon />
            <AlertContent>
              <AlertDescription>{error}</AlertDescription>
            </AlertContent>
          </Alert>
        ) : null}
        {status !== null ? (
          <Alert variant="info">
            <AlertContent>
              <AlertDescription>{status}</AlertDescription>
            </AlertContent>
          </Alert>
        ) : null}

        {stats !== null ? (
          <StatGrid>
            <StatTile label="交易笔数" value={String(stats.trades)} />
            <StatTile label="胜率" value={pct(stats.winRate)} />
            <StatTile
              label={
                <>
                  <TermHint termKey="r" /> 平均 R 倍数
                </>
              }
              value={stats.avgR.toFixed(2)}
            />
            <StatTile label="总盈亏" value={stats.totalPnl.toFixed(2)} />
          </StatGrid>
        ) : null}

        <Alert variant="info">
          <AlertContent>
            <AlertDescription>
              统计口径：{STATS_NOTE}。
              {feeOn
                ? `已逐笔扣减费用合计 ${stats === null ? "—" : stats.totalFees.toFixed(2)} ${currency}。`
                : `可在设置页打开费用模型，打开后逐笔扣减${feeItems(market)}。`}
            </AlertDescription>
          </AlertContent>
        </Alert>

        {view === null || view.entries.length === 0 ? (
          <p className="text-note text-ink-soft">还没有交易记录。</p>
        ) : (
          <DataTable>
            <thead>
              <tr>
                <Th>标的</Th>
                <Th>入场日</Th>
                <Th align="right">买入价</Th>
                <Th align="right">止损价</Th>
                <Th>出场日</Th>
                <Th align="right">出场价</Th>
                <Th align="right">R 倍数</Th>
                <Th align="right">盈亏</Th>
                <Th align="right">费用</Th>
                <Th>备注</Th>
              </tr>
            </thead>
            <tbody>
              {view.entries.map((entry) => (
                <Tr key={entry.id}>
                  <Td>
                    {entry.code} {entry.name ?? ""}
                  </Td>
                  <Td>{entry.entryDate}</Td>
                  <Td align="right">{price(entry.entryPrice)}</Td>
                  <Td align="right">{price(entry.stopPrice)}</Td>
                  <Td>{entry.exitDate ?? "持有中"}</Td>
                  <Td align="right">{price(entry.exitPrice)}</Td>
                  <Td align="right">
                    {entry.rMultiple === null ? "—" : entry.rMultiple.toFixed(2)}
                  </Td>
                  <Td align="right">{entry.pnl === null ? "—" : entry.pnl.toFixed(2)}</Td>
                  <Td align="right">{entry.fees.toFixed(2)}</Td>
                  <Td>{entry.note ?? ""}</Td>
                </Tr>
              ))}
            </tbody>
          </DataTable>
        )}

        <SectionHead>手动补录历史交易</SectionHead>
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <Input
            label="标的"
            value={form.code}
            onChange={(value) => setForm({ ...form, code: value })}
          />
          <Input
            label="入场日"
            type="date"
            value={form.entryDate}
            onChange={(value) => setForm({ ...form, entryDate: value })}
          />
          <Input
            label="买入价"
            type="number"
            step="0.01"
            value={String(form.entryPrice)}
            onChange={(value) => setForm({ ...form, entryPrice: Number(value) })}
          />
          <Input
            label="止损价"
            type="number"
            step="0.01"
            value={String(form.stopPrice)}
            onChange={(value) => setForm({ ...form, stopPrice: Number(value) })}
          />
          <Input
            label="股数"
            type="number"
            value={String(form.quantity)}
            onChange={(value) => setForm({ ...form, quantity: Number(value) })}
          />
          <Input
            label="出场日"
            type="date"
            value={form.exitDate ?? ""}
            onChange={(value) => setForm({ ...form, exitDate: value === "" ? null : value })}
          />
          <Input
            label="出场价"
            type="number"
            step="0.01"
            value={form.exitPrice === null ? "" : String(form.exitPrice)}
            onChange={(value) =>
              setForm({ ...form, exitPrice: value === "" ? null : Number(value) })
            }
          />
          <Input
            label="备注"
            value={form.note ?? ""}
            onChange={(value) => setForm({ ...form, note: value })}
          />
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <StatefulButton
            size="sm"
            disabled={form.code.trim() === "" || !filled}
            onClick={() => submit()}
          >
            补录
          </StatefulButton>
          <Button size="sm" variant="ghost" onClick={() => setFilled((value) => !value)}>
            {filled ? "取消补录" : "开始补录"}
          </Button>
          <span className="text-note text-ink-soft">
            出场价与出场日要一起填，或者都不填（都不填表示这笔还没出场）。
          </span>
        </div>

        {beginnerMode ? (
          <p className="text-note text-ink-soft">
            <TermHint termKey="slippage" />
            会让实际成交价劣于预期，日志里的盈亏是理论值。
          </p>
        ) : null}
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{riskNotes.costFree}</AlertDescription>
          </AlertContent>
        </Alert>
      </Panel>

      <JournalReview />
    </div>
  );
}

/** AI 复盘总结：单独一块，失败时可以重试。 */
function JournalReview() {
  const [text, setText] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const explanation = await journalReview();
      setText(explanation.text);
      setFailed(false);
    } catch {
      setFailed(true);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <Panel
      title="复盘总结"
      action={
        <Stamp status="info" size="sm">
          AI 生成
        </Stamp>
      }
    >
      {loading ? <Typesetting>正在生成…</Typesetting> : null}
      {failed ? (
        <div className="flex items-center gap-3">
          <Alert variant="destructive">
            <AlertIcon />
            <AlertContent>
              <AlertDescription>解释生成失败</AlertDescription>
            </AlertContent>
          </Alert>
          <Button size="sm" onClick={() => void load()}>
            重试
          </Button>
        </div>
      ) : null}
      {!loading && !failed && text !== null ? (
        <p className="text-lede leading-relaxed text-ink">{text}</p>
      ) : null}
      <p className="text-note text-ink-soft">
        以上只是对已经发生的结果的描述，不构成买卖建议。
      </p>
    </Panel>
  );
}
