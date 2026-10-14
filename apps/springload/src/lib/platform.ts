/**
 * 当前窗口是不是 macOS 的「隐藏标题栏」样式。
 *
 * 只有 macOS 上红黄绿三个按钮会浮在网页内容上，报头得给它们让出一条天条；
 * 其他平台的标题栏由系统画，不需要让。
 *
 * 这里读 UA 而不去问 Tauri，是因为它只决定一段留白：判错最多是样子不对，
 * 不值得为一个装饰性判断去改 `app_info` 这个已经冻结的接口。
 * 如果以后在 `tauri.conf.json` 里覆盖了 `userAgent`，这里要跟着改。
 */
export function isMacOS(): boolean {
  return navigator.userAgent.includes("Mac OS X");
}
