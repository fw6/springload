import type { ReactNode } from "react";
import { useBeginnerMode } from "../copy/BeginnerModeContext";

/** 悬停提示：一句话解释，鼠标停上去或键盘聚焦时显示。小白模式关闭时不显示。 */
export function Tooltip({ text, children }: { text: string; children: ReactNode }) {
  const { beginnerMode } = useBeginnerMode();
  if (!beginnerMode) return <>{children}</>;

  return (
    <span className="tip" tabIndex={0}>
      {children}
      <span className="tip-body" role="tooltip">
        {text}
      </span>
    </span>
  );
}
