import { useEffect, useState, useSyncExternalStore } from "react";
import { toMessage } from "../../api/invoke";
import { supportRefs, type ZoneRefs } from "../../api/zone";
import type { SupportZoneView } from "../../api/types";
import { AiCard } from "../../components/AiCard";
import { ConfidencePill } from "../../components/ConfidencePill";
import { getZones, loadZones, removeZone, subscribeZones } from "../../chart/zoneStore";
import type { StepProps } from "../useFlow";

/** 有置信度的标注类型；水平线只是一条参考线，不参与置信度。 */
function isBox(zone: SupportZoneView): boolean {
  return zone.kind === "support" || zone.kind === "gap";
}

function zoneLabel(zone: SupportZoneView): string {
  if (zone.kind === "gap") return "缺口";
  if (zone.kind === "hline") return "水平线";
  return "支撑区";
}

/**
 * 第 2 步面板：列出每条标注的下沿、上沿与置信度、三个参考价各自的落框数，
 * 接上 AI 解释卡片，并在没有高置信支撑区时挡住「进入第 3 步」。
 */
export default function Step2({ flow }: StepProps) {
  const { selected, avwapStartDate } = flow;
  const snapshot = useSyncExternalStore(subscribeZones, getZones, getZones);
  // 只展示当前这只股票的标注；切换股票的瞬间不会串到上一只的标注。
  const zones = snapshot.code === selected ? snapshot.zones : [];
  const [refs, setRefs] = useState<ZoneRefs | null>(null);
  const [refsError, setRefsError] = useState<string | null>(null);

  useEffect(() => {
    if (selected !== null) void loadZones(selected);
  }, [selected]);

  useEffect(() => {
    if (selected === null) {
      setRefs(null);
      setRefsError(null);
      return;
    }
    let active = true;
    supportRefs(selected, avwapStartDate)
      .then((result) => {
        if (!active) return;
        setRefs(result);
        setRefsError(null);
      })
      .catch((error) => {
        if (!active) return;
        setRefs(null);
        setRefsError(toMessage(error));
      });
    return () => {
      active = false;
    };
  }, [selected, avwapStartDate]);

  const refValues =
    refs === null
      ? []
      : [refs.ema21, refs.avwap, refs.prevHigh].filter(
          (value): value is number => value !== null && Number.isFinite(value),
        );
  const boxes = zones.filter(isBox);
  const lines = zones.filter((zone) => zone.kind === "hline");
  const hasHighConfidence = boxes.some((zone) => zone.confidence === "高");
  // 标注指纹：增删或拖动之后值会变，让 AI 解释缓存失效重算。
  const fingerprint = zones
    .map((zone) => `${zone.id}:${zone.low}:${zone.high}:${zone.confidence}`)
    .join("|");
  const hitsOf = (zone: SupportZoneView): number =>
    refValues.filter((value) => value >= zone.low && value <= zone.high).length;
  const boxesContaining = (value: number): number =>
    boxes.filter((zone) => value >= zone.low && value <= zone.high).length;

  return (
    <>
      <p className="hint">
        在左边图上用「支撑区」工具拖出矩形，用「水平线」放一条线，用「AVWAP 起点」指定起点。
        拖动矩形会实时重算置信度。
      </p>

      <h3 className="block-title">已标注</h3>
      {zones.length === 0 ? (
        <p className="hint">还没有标注。点上面的「支撑区」在图上拖一个框出来。</p>
      ) : (
        <table className="grid">
          <thead>
            <tr>
              <th className="left">标注</th>
              <th>下沿</th>
              <th>上沿</th>
              <th className="left">置信度</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {boxes.map((zone) => (
              <tr key={zone.id}>
                <td className="left">
                  <span className="pill" data-tone={zone.kind}>
                    {zoneLabel(zone)}
                  </span>
                </td>
                <td className="mono">{zone.low.toFixed(2)}</td>
                <td className="mono">{zone.high.toFixed(2)}</td>
                <td className="left">
                  <ConfidencePill level={zone.confidence} />
                  <span style={{ color: "#64748b", marginLeft: 8 }}>
                    {refs === null ? "参考价数据不足" : `参考价命中 ${hitsOf(zone)}/3`}
                  </span>
                </td>
                <td>
                  <button type="button" className="tool" onClick={() => void removeZone(zone.id)}>
                    删除
                  </button>
                </td>
              </tr>
            ))}
            {lines.map((zone) => (
              <tr key={zone.id}>
                <td className="left">
                  <span className="pill">水平线</span>
                </td>
                <td className="mono" colSpan={2}>
                  {zone.low.toFixed(2)}
                </td>
                <td className="left" style={{ color: "#64748b" }}>
                  拖动可改价格
                </td>
                <td>
                  <button type="button" className="tool" onClick={() => void removeZone(zone.id)}>
                    删除
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h3 className="block-title">三个参考价</h3>
      <table className="grid">
        <tbody>
          {[
            { label: "EMA21", value: refs?.ema21 ?? null },
            { label: "AVWAP", value: refs?.avwap ?? null },
            { label: "前期高点", value: refs?.prevHigh ?? null },
          ].map((row) => (
            <tr key={row.label}>
              <td className="left">{row.label}</td>
              <td className="mono">{row.value === null ? "—" : row.value.toFixed(2)}</td>
              <td className="left" style={{ color: "#64748b" }}>
                {row.value === null
                  ? "数据不足"
                  : boxesContaining(row.value) === 0
                    ? "不在任何框内"
                    : `落在 ${boxesContaining(row.value)} 个框内`}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {snapshot.error !== null ? (
        <div className="notice" data-tone="warn">
          {snapshot.error}
        </div>
      ) : null}
      {refsError !== null ? (
        <div className="notice" data-tone="warn">
          {refsError}
        </div>
      ) : null}

      <AiCard code={selected} scope={2} invalidateToken={fingerprint} />

      <div className="panel-foot">
        <button
          type="button"
          className="primary"
          disabled={!hasHighConfidence}
          onClick={() => flow.goStep(3)}
        >
          完成标注，进入第 3 步
        </button>
        {hasHighConfidence ? null : <span className="hint">至少需要一个高置信支撑区</span>}
      </div>
    </>
  );
}
