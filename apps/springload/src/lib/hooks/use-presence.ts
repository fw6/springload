"use client";

import { useEffect, useState } from "react";

/**
 * 退场期间保持挂载。
 *
 * `open` 变回 false 之后，节点还要留 `exitMs` 毫秒让 CSS 把收起动画放完，留满
 * 再摘掉。`exitMs` 为 0 时立即摘掉。中间又打开的话，计时作废，节点一直留着。
 */
export function usePresence(open: boolean, exitMs: number): boolean {
  const [present, setPresent] = useState(open);

  useEffect(() => {
    if (open) {
      setPresent(true);
      return;
    }
    if (exitMs <= 0) {
      setPresent(false);
      return;
    }
    const timer = window.setTimeout(() => setPresent(false), exitMs);
    return () => window.clearTimeout(timer);
  }, [open, exitMs]);

  return present;
}
