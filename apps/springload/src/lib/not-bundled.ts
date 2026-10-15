/**
 * 占位模块：打包时把 jsPDF 用不到的可选依赖指到这里。
 *
 * jsPDF 的 `html()` 与 `svg()` 会动态 import html2canvas / canvg / dompurify，本
 * 应用只用 `addImage()` 把 PNG 放进 PDF，这三条路径都不会走到，指过来之后它们不
 * 进包（少 389K）。真被调到就地抛出，而不是导出的 PDF 悄悄少一块。
 */
throw new Error("这个构建没有打包 jsPDF 的 html()/svg() 依赖");
