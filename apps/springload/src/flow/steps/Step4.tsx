import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toMessage } from "../../api/invoke";
import { planSave } from "../../api/plan";
import { getSettings } from "../../api/settings";
import { planCalc, type SizingResult } from "../../api/sizing";
import { AiCard } from "../../components/AiCard";
import { TermHint } from "../../components/TermHint";
import type { StepProps } from "../useFlow";

/** 写死的示例面板：账户 100,000 元，买入价 20.00、止损价 19.50。 */
const EXAMPLE = { entry: 20.0, stop: 19.5, account: 100_000, riskPct: 0.005 };

/** 金额按元展示，两位小数。 */
function money(value: number): string {
  return value.toFixed(2);
}

/** 比例按百分数展示，两位小数。 */
function pct(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

/** 按 A 股整股取整算减仓股数：3R 与 5R 各卖 floor(股数 ÷ 3) 股，剩余跟随 9 日均线。 */
export function reduceShares(shares: number): {
  shares3r: number;
  shares5r: number;
  remaining: number;
} {
  const safe = Math.max(0, Math.floor(shares));
  const each = Math.floor(safe / 3);
  return { shares3r: each, shares5r: each, remaining: safe - 2 * each };
}

/** 第 4 步：输入买入价、止损价、账户金额与单笔风险比例，实时算出股数与目标位。 */
export default function Step4({ flow }: StepProps) {
  const { sizing, setSizing } = flow;
  const navigate = useNavigate();
  const [result, setResult] = useState<SizingResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [warnDistance, setWarnDistance] = useState(0.05);

  /** 把当前输入存成一条计划，然后跳到计划卡页面。 */
  const generateCard = async () => {
    if (flow.selected === null) return;
    setBusy(true);
    try {
      await planSave({
        code: flow.selected,
        entry: sizing.entry,
        stop: sizing.stop,
        account: sizing.account,
        riskPct: sizing.riskPct,
        zoneId: flow.signal?.zoneId ?? null,
      });
      setPlanError(null);
      navigate("/plan");
    } catch (cause: unknown) {
      setPlanError(toMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((settings) => {
        if (!cancelled) setWarnDistance(settings.stopDistanceWarn);
      })
      .catch(() => {
        // 取不到配置时沿用默认阈值，界面上的警告判断照常给出。
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    planCalc(sizing)
      .then((next) => {
        if (cancelled) return;
        setResult(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        if (cancelled) return;
        setResult(null);
        setError(toMessage(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [sizing]);

  const reduce = result === null ? null : reduceShares(result.shares);
  const overThreshold = result !== null && result.stopDistance > warnDistance;

  return (
    <>
      <p className="hint">
        <TermHint termKey="r" />：买入价减止损价。股数按 A 股一手 100 股向下取整。
      </p>

      <div className="field-row">
        <div className="field">
          <label htmlFor="sizing-entry">买入价（元/股）</label>
          <input
            id="sizing-entry"
            type="number"
            step="0.01"
            value={sizing.entry}
            onChange={(event) => setSizing({ ...sizing, entry: Number(event.target.value) })}
          />
        </div>
        <div className="field">
          <label htmlFor="sizing-stop">止损价（元/股）</label>
          <input
            id="sizing-stop"
            type="number"
            step="0.01"
            value={sizing.stop}
            onChange={(event) => setSizing({ ...sizing, stop: Number(event.target.value) })}
          />
        </div>
        <div className="field">
          <label htmlFor="sizing-account">账户金额（元）</label>
          <input
            id="sizing-account"
            type="number"
            value={sizing.account}
            onChange={(event) => setSizing({ ...sizing, account: Number(event.target.value) })}
          />
        </div>
        <div className="field">
          <label htmlFor="sizing-risk">单笔风险比例（%）</label>
          <input
            id="sizing-risk"
            type="number"
            step="0.1"
            value={sizing.riskPct * 100}
            onChange={(event) => setSizing({ ...sizing, riskPct: Number(event.target.value) / 100 })}
          />
        </div>
      </div>

      {error !== null ? (
        <div className="notice" data-tone="risk">
          {error}
        </div>
      ) : null}

      {result !== null ? (
        <>
          <div className="result-grid">
            <dl className="result-cell">
              <dt>可买股数</dt>
              <dd className="mono">{result.shares}</dd>
            </dl>
            <dl className="result-cell">
              <dt>投入金额</dt>
              <dd className="mono">{money(result.capital)}</dd>
            </dl>
            <dl className="result-cell">
              <dt>仓位占比</dt>
              <dd className="mono">{pct(result.posPct)}</dd>
            </dl>
            <dl className="result-cell">
              <dt>止损距离</dt>
              <dd className="mono">{pct(result.stopDistance)}</dd>
            </dl>
            <dl className="result-cell">
              <dt>
                <TermHint termKey="target">3R 目标位</TermHint>
              </dt>
              <dd className="mono">{money(result.t3)}</dd>
            </dl>
            <dl className="result-cell">
              <dt>5R 目标位</dt>
              <dd className="mono">{money(result.t5)}</dd>
            </dl>
          </div>

          <div className="formula">
            每股风险 = {money(sizing.entry)} − {money(sizing.stop)} = <b>{money(result.perR)}</b>
            <br />
            最大可亏金额 = {money(sizing.account)} × {pct(sizing.riskPct)} ={" "}
            <b>{money(result.maxLoss)}</b>
            <br />
            可买股数 = {money(result.maxLoss)} ÷ {money(result.perR)} 后向下取整到 100 股 ={" "}
            <b>{result.shares} 股</b>
            <br />
            投入金额 = {result.shares} × {money(sizing.entry)} = <b>{money(result.capital)}</b>
            <br />
            仓位占比 = {money(result.capital)} ÷ {money(sizing.account)} = <b>{pct(result.posPct)}</b>
            <br />
            止损距离 = {money(result.perR)} ÷ {money(sizing.entry)} = <b>{pct(result.stopDistance)}</b>
            <br />
            3R 目标位 = {money(sizing.entry)} + 3 × {money(result.perR)} = <b>{money(result.t3)}</b>
            <br />
            5R 目标位 = {money(sizing.entry)} + 5 × {money(result.perR)} = <b>{money(result.t5)}</b>
          </div>

          {overThreshold ? (
            <div className="notice" data-tone="warn">
              止损距离 {pct(result.stopDistance)} 超过 {pct(warnDistance)}
              ，超出这套策略建议的 3%~5%。不阻止提交，请自行决定。
            </div>
          ) : null}
          {result.shares === 0 ? (
            <div className="notice" data-tone="warn">
              股数算出来是 0：最大可亏金额买不到一手 100 股。可能是止损位太近，也可能是风险比例太小。
            </div>
          ) : null}
        </>
      ) : null}

      {reduce !== null ? (
        <div className="notice" data-tone="info">
          减仓安排：3R 卖 {reduce.shares3r} 股、5R 再卖 {reduce.shares5r} 股，剩余{" "}
          {reduce.remaining} 股跟随 9 日均线。按整股取整，不按手取整。
        </div>
      ) : null}

      {result !== null ? (
        <div className="panel-foot">
          <button
            type="button"
            className="primary"
            onClick={() => void generateCard()}
            disabled={busy || result.shares <= 0}
          >
            {busy ? "正在生成…" : "生成计划卡"}
          </button>
          <span className="hint">未用满的风险金额 {money(result.unusedRisk)} 元</span>
        </div>
      ) : null}
      {planError !== null ? (
        <div className="notice" data-tone="risk">
          {planError}
        </div>
      ) : null}

      <div className="block-title">示例面板（写死数字，可用来对照自己的结果）</div>
      <div className="hint mono">
        账户 {EXAMPLE.account} 元，买入价 {money(EXAMPLE.entry)}、止损价 {money(EXAMPLE.stop)}
        → 每股风险 0.50、最大可亏 500.00、可买 1000 股、投入 20000.00 元（占 20.00%）、止损距离
        2.50%、3R 21.50、5R 22.50
      </div>

      <AiCard code={flow.selected} scope={4} />
    </>
  );
}
