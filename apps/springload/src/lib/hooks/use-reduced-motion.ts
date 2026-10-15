"use client";

import { useSyncExternalStore } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

function subscribe(onChange: () => void) {
  const media = window.matchMedia(QUERY);
  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}

/**
 * 用户是否要求减少动效。
 *
 * 视觉那一半由 index.css 末尾的全局规则压成零秒，这里只给还要按毫秒数收尾的
 * 地方用：动画已经没有时长了，节点就不该再为它多留一段时间。
 */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia(QUERY).matches,
    () => false,
  );
}
