import { useCallback, useEffect, useState } from "react";
import { toMessage } from "../api/invoke";
import {
  journalAddManual,
  journalExportCsv,
  journalGet,
  journalReview,
  type JournalView,
  type ManualEntry,
} from "../api/journal";
import { TermHint } from "../components/TermHint";
import { useBeginnerMode } from "../copy/BeginnerModeContext";
import { riskNotes } from "../copy/riskNotes";

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
  const [view, setView] = useState<JournalView | null>(null);
  const [form, setForm] = useState<ManualEntry>(BLANK);
  const [filled, setFilled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
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
    async (action: () => Promise<string>) => {
      setBusy(true);
      try {
        setStatus(await action());
        setError(null);
        await load();
      } catch (cause: unknown) {
        setError(toMessage(cause));
      } finally {
        setBusy(false);
      }
    },
    [load],
  );

  const submit = useCallback(async () => {
    await run(async () => {
      await journalAddManual(form);
      setForm(BLANK);
      setFilled(false);
      return "已补录一笔历史交易。";
    });
  }, [form, run]);

  const exportCsv = useCallback(async () => {
    await run(async () => {
      const path = `springload-journal-${new Date().toISOString().slice(0, 10)}.csv`;
      await journalExportCsv(path);
      return `已导出 CSV：${path}`;
    });
  }, [run]);

  const stats = view?.stats ?? null;

  return (
    <div className="page">
      <div className="chart-card">
        <div className="panel-head">
          <h2>交易日志</h2>
          <span className="toolbar-spacer" />
          <span className="hint">
            {view?.feeModelEnabled === true
              ? `费用模型已开启（佣金费率 ${pct(view.commissionRate)}）`
              : "费用模型关闭"}
          </span>
          <button type="button" className="ghost" onClick={() => void exportCsv()} disabled={busy}>
            导出 CSV
          </button>
        </div>
        <div className="panel-body">
          {error !== null ? (
            <div className="notice" data-tone="risk">
              {error}
            </div>
          ) : null}
          {status !== null ? (
            <div className="notice" data-tone="info">
              {status}
            </div>
          ) : null}

          {stats !== null ? (
            <div className="result-grid">
              <dl className="result-cell">
                <dt>交易笔数</dt>
                <dd className="mono">{stats.trades}</dd>
              </dl>
              <dl className="result-cell">
                <dt>胜率</dt>
                <dd className="mono">{pct(stats.winRate)}</dd>
              </dl>
              <dl className="result-cell">
                <dt>
                  <TermHint termKey="r" /> 平均 R 倍数
                </dt>
                <dd className="mono">{stats.avgR.toFixed(2)}</dd>
              </dl>
              <dl className="result-cell">
                <dt>总盈亏</dt>
                <dd className="mono">{stats.totalPnl.toFixed(2)}</dd>
              </dl>
            </div>
          ) : null}

          <div className="notice" data-tone="info">
            统计口径：{STATS_NOTE}。
            {view?.feeModelEnabled === true
              ? `已逐笔扣减费用合计 ${stats === null ? "—" : stats.totalFees.toFixed(2)} 元。`
              : "可在设置页打开费用模型，打开后逐笔扣减印花税、过户费、经手费、证管费与佣金。"}
          </div>

          {view === null || view.entries.length === 0 ? (
            <div className="hint">还没有交易记录。</div>
          ) : (
            <table className="grid">
              <thead>
                <tr>
                  <th className="left">标的</th>
                  <th className="left">入场日</th>
                  <th>买入价</th>
                  <th>止损价</th>
                  <th className="left">出场日</th>
                  <th>出场价</th>
                  <th>R 倍数</th>
                  <th>盈亏</th>
                  <th>费用</th>
                  <th className="left">备注</th>
                </tr>
              </thead>
              <tbody>
                {view.entries.map((entry) => (
                  <tr key={entry.id}>
                    <td className="left mono">
                      {entry.code} {entry.name ?? ""}
                    </td>
                    <td className="left mono">{entry.entryDate}</td>
                    <td className="mono">{price(entry.entryPrice)}</td>
                    <td className="mono">{price(entry.stopPrice)}</td>
                    <td className="left mono">{entry.exitDate ?? "持有中"}</td>
                    <td className="mono">{price(entry.exitPrice)}</td>
                    <td className="mono">
                      {entry.rMultiple === null ? "—" : entry.rMultiple.toFixed(2)}
                    </td>
                    <td className="mono">{entry.pnl === null ? "—" : entry.pnl.toFixed(2)}</td>
                    <td className="mono">{entry.fees.toFixed(2)}</td>
                    <td className="left">{entry.note ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          <div className="block-title">手动补录历史交易</div>
          <div className="field-row">
            <div className="field">
              <label htmlFor="manual-code">标的</label>
              <input
                id="manual-code"
                value={form.code}
                onChange={(event) => setForm({ ...form, code: event.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="manual-entry-date">入场日</label>
              <input
                id="manual-entry-date"
                type="date"
                value={form.entryDate}
                onChange={(event) => setForm({ ...form, entryDate: event.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="manual-entry">买入价</label>
              <input
                id="manual-entry"
                type="number"
                step="0.01"
                value={form.entryPrice}
                onChange={(event) => setForm({ ...form, entryPrice: Number(event.target.value) })}
              />
            </div>
            <div className="field">
              <label htmlFor="manual-stop">止损价</label>
              <input
                id="manual-stop"
                type="number"
                step="0.01"
                value={form.stopPrice}
                onChange={(event) => setForm({ ...form, stopPrice: Number(event.target.value) })}
              />
            </div>
            <div className="field">
              <label htmlFor="manual-quantity">股数</label>
              <input
                id="manual-quantity"
                type="number"
                value={form.quantity}
                onChange={(event) => setForm({ ...form, quantity: Number(event.target.value) })}
              />
            </div>
          </div>

          <div className="field-row">
            <div className="field">
              <label htmlFor="manual-exit-date">出场日</label>
              <input
                id="manual-exit-date"
                type="date"
                value={form.exitDate ?? ""}
                onChange={(event) =>
                  setForm({ ...form, exitDate: event.target.value === "" ? null : event.target.value })
                }
              />
            </div>
            <div className="field">
              <label htmlFor="manual-exit">出场价</label>
              <input
                id="manual-exit"
                type="number"
                step="0.01"
                value={form.exitPrice ?? ""}
                onChange={(event) =>
                  setForm({
                    ...form,
                    exitPrice: event.target.value === "" ? null : Number(event.target.value),
                  })
                }
              />
            </div>
            <div className="field">
              <label htmlFor="manual-note">备注</label>
              <input
                id="manual-note"
                value={form.note ?? ""}
                onChange={(event) => setForm({ ...form, note: event.target.value })}
              />
            </div>
            <div className="field">
              <button
                type="button"
                className="primary"
                onClick={() => void submit()}
                disabled={busy || form.code.trim() === "" || !filled}
              >
                补录
              </button>
            </div>
          </div>
          <p className="hint">
            出场价与出场日要一起填，或者都不填（都不填表示这笔还没出场）。
            <button type="button" className="ghost" onClick={() => setFilled((value) => !value)}>
              {filled ? "取消补录" : "开始补录"}
            </button>
          </p>

          {beginnerMode ? (
            <p className="hint">
              <TermHint termKey="slippage" />
              会让实际成交价劣于预期，日志里的盈亏是理论值。
            </p>
          ) : null}
          <div className="notice" data-tone="risk">
            {riskNotes.costFree}
          </div>
        </div>
      </div>

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
    <div className="chart-card">
      <div className="panel-head">
        <h2>复盘总结</h2>
        <span className="toolbar-spacer" />
        <span className="pill" data-tone="info">
          AI 生成
        </span>
      </div>
      <div className="panel-body">
        {loading ? <div className="hint">正在生成…</div> : null}
        {failed ? (
          <>
            <div className="notice" data-tone="risk">
              解释生成失败
            </div>
            <button type="button" className="primary" onClick={() => void load()}>
              重试
            </button>
          </>
        ) : null}
        {!loading && !failed && text !== null ? <p>{text}</p> : null}
        <span className="hint">以上只是对已经发生的结果的描述，不构成买卖建议。</span>
      </div>
    </div>
  );
}
