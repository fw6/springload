import { describe, expect, it } from "vitest";

/** 1×1 的 PNG。 */
const PNG_1X1 =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mM4IafxHwAEzgIOYs+UpQAAAABJRU5ErkJggg==";

/**
 * 打包时把 jsPDF 的可选依赖指到了 not-bundled（见 vite.config.ts），省 389K。
 * 那条排除只在「本应用只用 addImage 放 PNG」的前提下成立，这里按导出的用法
 * 走一遍，哪天 jsPDF 的这条路径改用 html2canvas / canvg / dompurify，会在这里
 * 就地失败。
 */
describe("jsPDF 放 PNG", () => {
  it("addImage 加 save 不碰被排除的那几个库", async () => {
    const { jsPDF } = await import("jspdf");
    const pdf = new jsPDF({ orientation: "portrait", unit: "px", format: [10, 10] });

    pdf.addImage(PNG_1X1, "PNG", 0, 0, 10, 10);

    const bytes = pdf.output("arraybuffer") as ArrayBuffer;
    expect(bytes.byteLength).toBeGreaterThan(0);
  });
});
