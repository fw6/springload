"use client";

import type { ComponentProps, ReactNode, Ref } from "react";
import { cn } from "@/lib/utils";

/** 只负责画：定位与测量交给外层的 positioner。 */
export function TooltipSurface({
  children,
  side: _side = "top",
  className,
  ref,
  ready = true,
  state,
  style,
  ...props
}: Omit<ComponentProps<"span">, "children"> & {
  children?: ReactNode;
  /** 定位层量完之后再放行入场动画。 */
  ready?: boolean;
  side?: "top" | "right" | "bottom" | "left";
  /** 收起中的那一份要留着放完退场动画。 */
  state: "open" | "closed";
  ref?: Ref<HTMLSpanElement>;
}) {
  const closing = state === "closed";
  return (
    <span
      ref={ref}
      role="tooltip"
      data-state={state}
      style={{ transformOrigin: "var(--tooltip-origin, center)", ...style }}
      className={cn(
        "block border border-ink bg-paper px-2.5 py-1.5 text-note leading-relaxed text-ink",
        closing ? "tooltip-out" : ready ? "tooltip-in" : "tooltip-before",
        className,
      )}
      {...props}
    >
      {children}
    </span>
  );
}
