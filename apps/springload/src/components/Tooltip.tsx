import type { ReactNode } from "react";
import { useBeginnerMode } from "../copy/BeginnerModeContext";
import { Tooltip as HoverTooltip } from "./ui/tooltip";

/**
 * 悬停提示：一句话解释，鼠标停上去或键盘聚焦时显示。小白模式关闭时不显示。
 *
 * 传进来的那个元素本身就是触发器，不额外包一层容器 —— 包一层会让事件挂在
 * 外层元素上，鼠标落在内层文字上时反而收不到。
 */
export function Tooltip({ text, children }: { text: string; children: ReactNode }) {
  const { beginnerMode } = useBeginnerMode();
  if (!beginnerMode) return <>{children}</>;

  return (
    <HoverTooltip
      content={text}
      side="top"
      delay={0}
      className="max-w-[18rem] whitespace-normal leading-relaxed"
    >
      {children}
    </HoverTooltip>
  );
}

/** 纯文字触发器上的虚线下划：告诉用户这个词下面还有解释。 */
export const TOOLTIP_TEXT_CLASS = "cursor-help border-b border-dotted border-rule";
