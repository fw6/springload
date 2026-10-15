import { afterEach, describe, expect, it } from "vitest";
import { isMacOS } from "./platform";

const original = navigator.userAgent;

function setUserAgent(value: string) {
  Object.defineProperty(navigator, "userAgent", { value, configurable: true });
}

afterEach(() => {
  setUserAgent(original);
});

describe("isMacOS", () => {
  it("WKWebView 的 UA 判成 macOS", () => {
    setUserAgent(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)",
    );
    expect(isMacOS()).toBe(true);
  });

  it("Windows 与 Linux 的 UA 都判成不是 macOS", () => {
    setUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko)");
    expect(isMacOS()).toBe(false);

    setUserAgent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)");
    expect(isMacOS()).toBe(false);
  });
});
