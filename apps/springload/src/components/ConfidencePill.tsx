import type { Confidence } from "../api/types";

/** 置信度胶囊。 */
export function ConfidencePill({ level }: { level: Confidence }) {
  return (
    <span className="pill" data-level={level}>
      {level}置信
    </span>
  );
}
