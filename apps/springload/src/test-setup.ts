// vitest 不开 globals：给 jsdom 补上 React 组件用到的浏览器 API。

import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// RTL 不会自动 cleanup，DOM 会跨用例泄漏。
afterEach(() => {
  cleanup();
});

if (!globalThis.matchMedia) {
  globalThis.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof globalThis.matchMedia;
}

if (!globalThis.ResizeObserver) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof globalThis.ResizeObserver;
}

if (!globalThis.IntersectionObserver) {
  globalThis.IntersectionObserver = class {
    root = null;
    rootMargin = "";
    thresholds: number[] = [];
    observe() {}
    unobserve() {}
    disconnect() {}
    takeRecords() {
      return [];
    }
  } as unknown as typeof globalThis.IntersectionObserver;
}

// jsdom 没有排版引擎：getBoundingClientRect 返回 0×0，offsetWidth/offsetHeight
// 恒为 0，靠测量驱动的组件（虚拟滚动表格）因此一行都不渲染。组件会把视口尺寸
// 写在行内样式上，这里按声明的尺寸回填，让测量拿到组件自己声明的盒子。
function declaredSize(element: Element, axis: "width" | "height"): number {
  const value = (element as HTMLElement).style?.[axis];
  if (!value) return 0;
  const size = Number.parseFloat(value);
  return Number.isFinite(size) ? Math.round(size) : 0;
}

for (const axis of ["width", "height"] as const) {
  const property = axis === "width" ? "offsetWidth" : "offsetHeight";
  Object.defineProperty(HTMLElement.prototype, property, {
    configurable: true,
    get(this: HTMLElement) {
      return declaredSize(this, axis);
    },
  });
}

const nativeRect = Element.prototype.getBoundingClientRect;
Element.prototype.getBoundingClientRect = function getBoundingClientRect(this: Element) {
  const rect = nativeRect.call(this);
  if (rect.width !== 0 || rect.height !== 0) return rect;
  const width = declaredSize(this, "width");
  const height = declaredSize(this, "height");
  if (width === 0 && height === 0) return rect;
  return new DOMRect(rect.x, rect.y, width, height);
};

// jsdom 有 <dialog> 的样式与 open 属性，但没有 showModal/close（也就没有顶层与
// inert 语义）。补上最小的开关行为，让对话框的打开与关闭能被测试驱动。
const dialogPrototype = globalThis.HTMLDialogElement?.prototype;
if (dialogPrototype && !dialogPrototype.showModal) {
  dialogPrototype.showModal = function showModal(this: HTMLDialogElement) {
    this.open = true;
  };
  dialogPrototype.show = function show(this: HTMLDialogElement) {
    this.open = true;
  };
  dialogPrototype.close = function close(this: HTMLDialogElement, returnValue?: string) {
    if (!this.open) return;
    this.open = false;
    if (returnValue !== undefined) this.returnValue = returnValue;
    this.dispatchEvent(new Event("close"));
  };
}
