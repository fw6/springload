import type { Confidence } from "../api/types";
import { Stamp, type StampStatus } from "./ui/stamp";

const STATUS: Record<Confidence, StampStatus> = {
  高: "success",
  中: "warning",
  低: "neutral",
};

/** 置信度戳记。 */
export function ConfidencePill({ level }: { level: Confidence }) {
  return (
    <Stamp status={STATUS[level]} size="sm" data-level={level}>
      {level}置信
    </Stamp>
  );
}
