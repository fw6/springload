import { useEffect, useState, useSyncExternalStore } from "react";
import { toMessage } from "../../api/invoke";
import { supportRefs, type ZoneRefs } from "../../api/zone";
import type { SupportZoneView } from "../../api/types";
import { AiCard } from "../../components/AiCard";
import { ConfidencePill } from "../../components/ConfidencePill";
import { DataTable, Td, Th, Tr } from "../../components/DataTable";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Stamp, type StampStatus } from "../../components/ui/stamp";
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

function zoneStatus(zone: SupportZoneView): StampStatus {
  if (zone.kind === "gap") return "warning";
  if (zone.kind === "hline") return "neutral";
  return "info";
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
 * 第 2 版：列出每条标注的下沿、上沿与置信度、三个参考价各自的落框数，
 * 接上 AI 解释，并在没有高置信支撑区时挡住「进入第 3 步」。
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
      <p className="text-body leading-relaxed text-ink-soft">
        在左图版上用「支撑区」工具拖出矩形，用「水平线」放一条线，用「AVWAP 起点」指定起点。
        拖动矩形会实时重算置信度。
      </p>

      <SectionHead>已标注</SectionHead>
      {zones.length === 0 ? (
        <p className="text-note text-ink-soft">
          还没有标注。点上面的「支撑区」在图上拖一个框出来。
        </p>
      ) : (
        <DataTable>
          <thead>
            <tr>
              <Th>标注</Th>
              <Th align="right">下沿</Th>
              <Th align="right">上沿</Th>
              <Th>置信度</Th>
              <Th />
            </tr>
          </thead>
          <tbody>
            {boxes.map((zone) => (
              <Tr key={zone.id}>
                <Td>
                  <Stamp status={zoneStatus(zone)} size="sm">
                    {zoneLabel(zone)}
                  </Stamp>
                </Td>
                <Td align="right">{zone.low.toFixed(2)}</Td>
                <Td align="right">{zone.high.toFixed(2)}</Td>
                <Td>
                  <span className="flex items-center gap-2">
                    <ConfidencePill level={zone.confidence} />
                    <span className="text-note text-ink-soft">
                      {refs === null ? "参考价数据不足" : `参考价命中 ${hitsOf(zone)}/3`}
                    </span>
                  </span>
                </Td>
                <Td align="right">
                  <Button size="sm" variant="ghost" onClick={() => void removeZone(zone.id)}>
                    删除
                  </Button>
                </Td>
              </Tr>
            ))}
            {lines.map((zone) => (
              <Tr key={zone.id}>
                <Td>
                  <Stamp status="neutral" size="sm">
                    水平线
                  </Stamp>
                </Td>
                <Td align="right" colSpan={2}>
                  {zone.low.toFixed(2)}
                </Td>
                <Td className="text-note text-ink-soft">拖动可改价格</Td>
                <Td align="right">
                  <Button size="sm" variant="ghost" onClick={() => void removeZone(zone.id)}>
                    删除
                  </Button>
                </Td>
              </Tr>
            ))}
          </tbody>
        </DataTable>
      )}

      <SectionHead>三个参考价</SectionHead>
      <DataTable>
        <tbody>
          {[
            { label: "EMA21", value: refs?.ema21 ?? null },
            { label: "AVWAP", value: refs?.avwap ?? null },
            { label: "前期高点", value: refs?.prevHigh ?? null },
          ].map((row) => (
            <Tr key={row.label}>
              <Td>{row.label}</Td>
              <Td align="right">{row.value === null ? "—" : row.value.toFixed(2)}</Td>
              <Td className="text-note text-ink-soft">
                {row.value === null
                  ? "数据不足"
                  : boxesContaining(row.value) === 0
                    ? "不在任何框内"
                    : `落在 ${boxesContaining(row.value)} 个框内`}
              </Td>
            </Tr>
          ))}
        </tbody>
      </DataTable>

      {snapshot.error !== null ? (
        <Alert variant="warning">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{snapshot.error}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}
      {refsError !== null ? (
        <Alert variant="warning">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{refsError}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      <AiCard code={selected} scope={2} invalidateToken={fingerprint} />

      <div className="flex flex-wrap items-center gap-3">
        <Button size="sm" disabled={!hasHighConfidence} onClick={() => flow.goStep(3)}>
          完成标注，进入第 3 步
        </Button>
        {hasHighConfidence ? null : (
          <span className="text-note text-ink-soft">至少需要一个高置信支撑区</span>
        )}
      </div>
    </>
  );
}
