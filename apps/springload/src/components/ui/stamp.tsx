import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "../../lib/utils";

export type StampStatus =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "loading";

export type StampSize = "sm" | "md";

/** 每种状态的墨色与底纹。 */
const TONES: Record<StampStatus, string> = {
  neutral: "border-rule text-ink-soft bg-slip",
  info: "border-ink text-ink bg-slip",
  success: "border-ok text-ok bg-slip",
  warning: "border-warn text-warn bg-warn-soft",
  danger: "border-alert bg-alert text-paper",
  loading: "border-rule text-ink-soft bg-slip",
};

const SIZES: Record<StampSize, string> = {
  sm: "h-5 px-1.5 text-micro",
  md: "h-6 px-2 text-note",
};

export interface StampProps extends ComponentPropsWithoutRef<"span"> {
  status?: StampStatus;
  size?: StampSize;
  children?: ReactNode;
  /** 状态切换时的重排标记，变化时重播一次压印。 */
  contentKey?: string | number;
}

/** 戳记：版面上一枚加了框的状态小签。 */
export function Stamp({
  status = "neutral",
  size = "sm",
  className,
  children,
  contentKey,
  ...rest
}: StampProps) {
  return (
    <span
      data-slot="stamp"
      data-status={status}
      data-key={contentKey}
      className={cn(
        "inline-flex shrink-0 items-center justify-center gap-1 border font-medium whitespace-nowrap",
        TONES[status],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      {children}
    </span>
  );
}
