import { useEffect, useState, useTransition } from "react";
import { useNavigate } from "react-router-dom";
import { toMessage } from "../../api/invoke";
import { planSave } from "../../api/plan";
import { getSettings } from "../../api/settings";
import { planCalc, type SizingResult } from "../../api/sizing";
import { useCurrency } from "../../copy/MarketContext";
import { AiCard } from "../../components/AiCard";
import { StatGrid, StatTile } from "../../components/Panel";
import { TermHint } from "../../components/TermHint";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../../components/ui/alert";
import { StatefulButton } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import type { StepProps } from "../useFlow";

/** 写死的示例面板：账户 100,000，买入价 20.00、止损价 19.50。 */
const EXAMPLE = { entry: 20.0, stop: 19.5, account: 100_000, riskPct: 0.005 };

/** 金额按本市场货币展示，两位小数。 */
function money(value: number): string {
  return value.toFixed(2);
}

/** 比例按百分数展示，两位小数。 */
function pct(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

/** 按整股取整算减仓股数：3R 与 5R 各卖 floor(股数 ÷ 3) 股，剩余跟随 9 日均线。
 * 两个市场都按整股算：卖出申报不受 A 股一手 100 股的限制。 */
export function reduceShares(shares: number): {
  shares3r: number;
  shares5r: number;
  remaining: number;
} {
  const safe = Math.max(0, Math.floor(shares));
  const each = Math.floor(safe / 3);
  return { shares3r: each, shares5r: each, remaining: safe - 2 * each };
}

/** 第 4 版：输入买入价、止损价、账户金额与单笔风险比例，实时算出股数与目标位。 */
export default function Step4({ flow }: StepProps) {
  const { sizing, setSizing } = flow;
  const currency = useCurrency();
  const navigate = useNavigate();
  const [result, setResult] = useState<SizingResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [busy, startBusy] = useTransition();
  const [warnDistance, setWarnDistance] = useState(0.05);

  /** 把当前输入存成一条计划，然后跳到计划卡。 */
  const generateCard = () => {
    if (flow.selected === null) return;
    startBusy(async () => {
      try {
        await planSave({
          code: flow.selected as string,
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
      }
    });
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
    // 还没填价格时不发请求：刚进这一版不该先弹一条报错。
    if (!(sizing.entry > 0 && sizing.stop > 0)) {
      setResult(null);
      setError(null);
      return;
    }
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

  /** 取整口径的说法，只认后端给的 lotSize：A 股是一手 100 股，美股按整股。 */
  const lotRounding = (size: number) => (size === 1 ? "整股" : `${size} 股的整数倍`);
  const reduce = result === null ? null : reduceShares(result.shares);
  const overThreshold = result !== null && result.stopDistance > warnDistance;

  return (
    <>
      <p className="text-body leading-relaxed text-ink-soft">
        <TermHint termKey="r" />：买入价减止损价。股数按最小申报单位向下取整，单位由
        当前市场决定。
      </p>

      <div className="grid grid-cols-2 gap-3">
        <Input
          label={`买入价（${currency}/股）`}
          type="number"
          step="0.01"
          value={String(sizing.entry)}
          onChange={(value) => setSizing({ ...sizing, entry: Number(value) })}
        />
        <Input
          label={`止损价（${currency}/股）`}
          type="number"
          step="0.01"
          value={String(sizing.stop)}
          onChange={(value) => setSizing({ ...sizing, stop: Number(value) })}
        />
        <Input
          label={`账户金额（${currency}）`}
          type="number"
          value={String(sizing.account)}
          onChange={(value) => setSizing({ ...sizing, account: Number(value) })}
        />
        <Input
          label="单笔风险比例（%）"
          type="number"
          step="0.1"
          value={String(sizing.riskPct * 100)}
          onChange={(value) => setSizing({ ...sizing, riskPct: Number(value) / 100 })}
        />
      </div>

      {result === null && error === null ? (
        <p className="text-note text-ink-soft">
          填入买入价与止损价之后，这里会算出股数、投入金额与目标位，并逐项列出公式。
        </p>
      ) : null}

      {error !== null ? (
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{error}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      {result !== null ? (
        <>
          <StatGrid>
            <StatTile label="可买股数" value={String(result.shares)} />
            <StatTile label="投入金额" value={money(result.capital)} />
            <StatTile label="仓位占比" value={pct(result.posPct)} />
            <StatTile label="止损距离" value={pct(result.stopDistance)} />
            <StatTile
              label={<TermHint termKey="target">3R 目标位</TermHint>}
              value={money(result.t3)}
            />
            <StatTile label="5R 目标位" value={money(result.t5)} />
          </StatGrid>

          <div
            data-slot="formula"
            className="flex flex-col border-y-2 border-ink bg-slip px-3 py-2 font-mono text-note leading-6 text-ink-soft"
          >
            <span>
              每股风险 = {money(sizing.entry)} − {money(sizing.stop)} ={" "}
              <b className="text-ink">{money(result.perR)}</b>
            </span>
            <span>
              最大可亏金额 = {money(sizing.account)} × {pct(sizing.riskPct)} ={" "}
              <b className="text-ink">{money(result.maxLoss)}</b>
            </span>
            <span>
              可买股数 = {money(result.maxLoss)} ÷ {money(result.perR)} 后向下取整到{" "}
              {lotRounding(result.lotSize)} ={" "}
              <b className="text-ink">{result.shares} 股</b>
            </span>
            <span>
              投入金额 = {result.shares} × {money(sizing.entry)} ={" "}
              <b className="text-ink">{money(result.capital)}</b>
            </span>
            <span>
              仓位占比 = {money(result.capital)} ÷ {money(sizing.account)} ={" "}
              <b className="text-ink">{pct(result.posPct)}</b>
            </span>
            <span>
              止损距离 = {money(result.perR)} ÷ {money(sizing.entry)} ={" "}
              <b className="text-ink">{pct(result.stopDistance)}</b>
            </span>
            <span>
              3R 目标位 = {money(sizing.entry)} + 3 × {money(result.perR)} ={" "}
              <b className="text-ink">{money(result.t3)}</b>
            </span>
            <span>
              5R 目标位 = {money(sizing.entry)} + 5 × {money(result.perR)} ={" "}
              <b className="text-ink">{money(result.t5)}</b>
            </span>
          </div>

          {overThreshold ? (
            <Alert variant="warning">
              <AlertIcon />
              <AlertContent>
                <AlertDescription>
                  止损距离 {pct(result.stopDistance)} 超过 {pct(warnDistance)}
                  ，超出这套策略建议的 3%~5%。不阻止提交，请自行决定。
                </AlertDescription>
              </AlertContent>
            </Alert>
          ) : null}
          {result.shares === 0 ? (
            <Alert variant="warning">
              <AlertIcon />
              <AlertContent>
                <AlertDescription>
                  股数算出来是 0：最大可亏金额买不到一个最小申报单位（
                  {lotRounding(result.lotSize)}）。可能是止损位太近，也可能是风险比例太小。
                </AlertDescription>
              </AlertContent>
            </Alert>
          ) : null}
        </>
      ) : null}

      {reduce !== null ? (
        <Alert variant="info">
          <AlertContent>
            <AlertDescription>
              减仓安排：3R 卖 {reduce.shares3r} 股、5R 再卖 {reduce.shares5r} 股，剩余{" "}
              {reduce.remaining} 股跟随 9 日均线。卖出按整股取整，不受买入申报单位的限制。
            </AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      {result !== null ? (
        <div className="flex flex-wrap items-center gap-3">
          <StatefulButton
            size="sm"
            state={busy ? "loading" : "idle"}
            loadingText="正在生成…"
            disabled={result.shares <= 0}
            onClick={() => generateCard()}
          >
            生成计划卡
          </StatefulButton>
          <span className="text-note text-ink-soft">
            未用满的风险金额 {money(result.unusedRisk)} {currency}
          </span>
        </div>
      ) : null}
      {planError !== null ? (
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{planError}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      <div className="border-b border-rule pb-1 font-display text-body font-bold text-ink">
        示例面板（写死数字，可用来对照自己的结果）
      </div>
      <div className="tabular text-note leading-relaxed text-ink-soft">
        账户 {EXAMPLE.account} {currency}，买入价 {money(EXAMPLE.entry)}、止损价{" "}
        {money(EXAMPLE.stop)}
        → 每股风险 0.50、最大可亏 500.00、可买 1000 股、投入 20000.00 {currency}（占
        20.00%）、止损距离
        2.50%、3R 21.50、5R 22.50
      </div>

      <AiCard code={flow.selected} scope={4} />
    </>
  );
}
