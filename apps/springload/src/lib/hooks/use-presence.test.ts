import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { usePresence } from "./use-presence";

describe("usePresence", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("关掉之后还留 exitMs，留满才摘", () => {
    const { result, rerender } = renderHook(({ open }) => usePresence(open, 260), {
      initialProps: { open: true },
    });
    expect(result.current).toBe(true);

    rerender({ open: false });
    expect(result.current).toBe(true);

    act(() => vi.advanceTimersByTime(259));
    expect(result.current).toBe(true);

    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toBe(false);
  });

  it("退场中间又打开：计时作废，节点一直留着", () => {
    const { result, rerender } = renderHook(({ open }) => usePresence(open, 260), {
      initialProps: { open: true },
    });
    rerender({ open: false });
    act(() => vi.advanceTimersByTime(100));
    rerender({ open: true });

    act(() => vi.advanceTimersByTime(1000));
    expect(result.current).toBe(true);
  });

  it("exitMs 为 0 时立即摘掉", () => {
    const { result, rerender } = renderHook(({ open }) => usePresence(open, 0), {
      initialProps: { open: true },
    });
    rerender({ open: false });
    expect(result.current).toBe(false);
  });
});
