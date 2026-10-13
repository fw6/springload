## Agent skills

### Issue tracker

Issues 与规格存放在本仓库的 GitHub Issues 中，用 `gh` 命令行读写。参见 `docs/agents/issue-tracker.md`。

### Triage labels

五个 triage 角色使用默认标签名：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`。参见 `docs/agents/triage-labels.md`。

### Domain docs

单上下文（single-context）布局：仓库根目录的 `GLOSSARY.md` 与 `docs/adr/`。参见 `docs/agents/domain.md`。

### 架构与接口契约

`docs/agents/architecture.md` 冻结了仓库骨架、Rust 模块归属、SQLite 表结构、Tauri 命令签名与前端文件归属。改动任何跨模块的接口之前先读它。
