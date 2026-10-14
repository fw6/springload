"use client";

import { motion, useReducedMotion } from "motion/react";
import type { ComponentProps, ReactNode, Ref } from "react";
import { EASE_OUT } from "@/lib/ease";
import { cn } from "@/lib/utils";

/** 只负责画：定位与测量交给外层的 positioner。 */
export function TooltipSurface({
  children,
  side: _side = "top",
  className,
  ref,
  ready = true,
  style,
  ...props
}: Omit<ComponentProps<typeof motion.span>, "children"> & {
  children?: ReactNode;
  /** 定位层量完之后再放行入场动画。 */
  ready?: boolean;
  side?: "top" | "right" | "bottom" | "left";
  ref?: Ref<HTMLSpanElement>;
}) {
  const reduce = useReducedMotion();
  const closed = { opacity: 0, scale: reduce ? 1 : 0.97 };
  return (
    <motion.span
      ref={ref}
      role="tooltip"
      initial={closed}
      animate={{
        ...(ready ? { opacity: 1, scale: 1 } : closed),
        transition: { duration: 0.16, ease: EASE_OUT },
      }}
      exit={{ ...closed, transition: { duration: 0.1, ease: EASE_OUT } }}
      style={{ transformOrigin: "var(--tooltip-origin, center)", ...style }}
      className={cn(
        "block border border-ink bg-paper px-2.5 py-1.5 text-note leading-relaxed text-ink",
        className,
      )}
      {...props}
    >
      {children}
    </motion.span>
  );
}
