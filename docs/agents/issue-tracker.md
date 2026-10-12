# Issue tracker：GitHub

本仓库的 issue 与规格存放在 GitHub Issues 中。所有操作使用 `gh` 命令行。

## 约定

- **创建 issue**：`gh issue create --title "..." --body "..."`。正文多行时使用 heredoc。
- **读取 issue**：`gh issue view <number> --json number,title,body,labels,comments`。
- **列出 issue**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，配合 `--label` 与 `--state` 过滤。
- **把 issue 设为某个父 issue 的子 issue**：`gh issue create --parent <parent> ...`，或事后 `gh issue edit <parent> --add-sub-issue <child>`（需要 `gh` 2.94+）。旧版 `gh`：`gh api --method POST repos/<owner>/<repo>/issues/<parent>/sub_issues -F sub_issue_id=<child-db-id>`（数据库 id，见下方**阻塞关系**）。如果环境不支持 sub-issue，就在子 issue 正文顶部写 `Part of #<parent>`。
- **评论 issue**：`gh issue comment <number> --body "..."`
- **添加或移除标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭**：`gh issue close <number> --comment "..."`

仓库由 `git remote -v` 推断；在克隆目录内运行 `gh` 时它会自动识别。

## Pull request 作为 triage 来源

**PRs as a request surface: no**（如果本仓库把外部 PR 当作功能请求，就改成 `yes`；`/triage` 会读这个标记。）

当该标记为 `yes` 时，PR 与 issue 使用同一套标签和状态，对应的 `gh pr` 命令：

- **读取 PR**：`gh pr view <number> --comments`，差异用 `gh pr diff <number>`。
- **列出待 triage 的外部 PR**：`gh api --paginate 'repos/{owner}/{repo}/pulls?state=open' --jq '.[] | select(.author_association | IN("OWNER","MEMBER","COLLABORATOR") | not) | {number, title, author: .user.login, author_association, labels: [.labels[].name]}'`。
- **评论、打标签、关闭**：`gh pr comment`、`gh pr edit --add-label` / `--remove-label`、`gh pr close`。

GitHub 的 issue 与 PR 共用一个编号空间，所以裸写的 `#42` 可能是 issue 也可能是 PR：先用 `gh pr view 42` 判断，失败再退回 `gh issue view 42`。

## 当某个技能说「publish to the issue tracker」

创建一个 GitHub issue。

## 当某个技能说「fetch the relevant ticket」

按上面**读取 issue**的方式读取。

## Wayfinding 操作

供 `/wayfinder` 使用。**地图**是一个 issue，**ticket** 是它的子 issue。

- **地图**：单个 issue，带 `wayfinder:map` 标签，正文存放 Notes / Decisions-so-far / Fog。`gh issue create --label wayfinder:map`。
- **子 ticket**：作为 GitHub sub-issue 挂到地图下（见**把 issue 设为某个父 issue 的子 issue**）。在不支持 sub-issue 的环境里，把子 issue 加进地图正文的任务清单，并在子 issue 正文顶部写 `Part of #<map>`。标签用 `wayfinder:<type>`（`research` / `prototype` / `grilling` / `task`）。被认领后，ticket 指派给驱动它的开发者。
- **阻塞关系**：使用 GitHub 的 native issue dependencies，这是标准的、在界面上可见的表示方式。添加一条边：`gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`，其中 `<blocker-db-id>` 是阻塞者的数字**数据库 id**（`gh api repos/<owner>/<repo>/issues/<n> --jq .id`，取到的既不是 `#number`，也不是 `node_id`）。GitHub 用 `issue_dependencies_summary.blocked_by` 报告仍然打开的阻塞者数量，这就是当前的闸门。在不支持 dependencies 的环境里，退回到子 issue 正文顶部的 `Blocked by: #<n>, #<n>` 一行。当所有阻塞者都关闭时，ticket 解除阻塞。
- **frontier 查询**：列出地图下仍然打开的 children（`gh issue list --state open`，范围限定在地图的 sub-issue 或任务清单内），去掉仍有打开阻塞者的（`issue_dependencies_summary.blocked_by > 0`，或 `Blocked by` 行里还有打开的 issue），再去掉已有指派人的；按地图顺序取第一个。
- **认领**：`gh issue edit <n> --add-assignee @me`，这是本次会话的第一次写操作。
- **解决**：`gh issue comment <n> --body "<answer>"`，然后 `gh issue close <n>`，再把上下文指针（要点加链接）追加到地图的 Decisions-so-far。
