# 域文档

工程技能在探索代码库时应如何消费本仓库的域文档。

## 探索之前先读这些

- 仓库根目录的 **`GLOSSARY.md`**，或者
- 如果存在仓库根目录的 **`GLOSSARY-MAP.md`**：它指向每个上下文各自的 `GLOSSARY.md`，读取与当前主题相关的那些。
- **`docs/adr/`**：读取与即将改动的区域相关的 ADR。多上下文仓库里还要看 `src/<context>/docs/adr/` 里的上下文级决策。

这些文件不存在时，**静默继续**。不要提示它们缺失，也不要主动建议创建。`/domain-modeling` 技能（经 `/grill-with-docs` 与 `/improve-codebase-architecture` 到达）会在术语或决策真正确定时按需创建。

## 文件结构

单上下文仓库（大多数仓库）：

- `GLOSSARY.md`
- `docs/adr/`，例如 `0001-event-sourced-orders.md`、`0002-postgres-for-write-model.md`

多上下文仓库（根目录存在 `GLOSSARY-MAP.md`）：

- `GLOSSARY-MAP.md`
- `docs/adr/`：系统级决策
- `src/ordering/GLOSSARY.md`、`src/ordering/docs/adr/`：上下文级决策
- `src/billing/GLOSSARY.md`、`src/billing/docs/adr/`

## 使用术语表里的词汇

当输出中要提到某个领域概念（issue 标题、重构提案、假设、测试名称）时，使用 `GLOSSARY.md` 里定义的词。不要漂移到术语表明确避免的同义词。

如果需要的概念还不在术语表里，这是一个信号：要么你在发明项目并不使用的说法（重新考虑），要么存在真实的缺口（记下来交给 `/domain-modeling`）。

## 标记 ADR 冲突

如果你的输出与已有 ADR 矛盾，明确提出来，不要悄悄覆盖：

> _与 ADR-0007（event-sourced orders）矛盾，但值得重新讨论，因为……_
