# 项目增强：性能、重构、测试与七项功能

## Goal

按 review 结论落地两组改进（各 1–7 项）并顺带排查 bug。功能 1（Dock 面板增强）沿用
`11-10-dock-panel-enhancements` 的 PRD / design，本任务只引用。

## A. 代码改进

| # | 项 | 交付 |
| --- | --- | --- |
| A1 | FTS 更新触发器只在 `search_text` / `note` 变化时重建 | 新 migration `0002`，不回改 `0001` |
| A2 | 列表排序 / 过滤索引 | 同 `0002` |
| A3 | 分页开销 | 前端是按 index 随机访问的 range cache，keyset 不适用；改为索引覆盖排序 + LIKE 多词 AND 语义 |
| A4 | 搜索相关度排序 | 新设置 `clipboard.search.sortByRelevance`（默认 false），FTS 命中时置顶后按 `bm25` |
| A5 | `commands/clipboard.rs` 业务下沉 | 列表增强 / 预览 / 分组校验移到 `clipboard/` 子模块 |
| A6 | 前端大文件拆分 | `List.tsx`、`Group.tsx` 拆出 hook / 子组件，行为不变 |
| A7 | 前端测试 | 引入 vitest，覆盖纯函数工具 |

## B. 功能

| # | 功能 | 要点 |
| --- | --- | --- |
| B1 | Dock 面板增强 | 见 `11-10-dock-panel-enhancements/prd.md` |
| B2 | 多选与批量操作 | Cmd/Ctrl+点击、Shift 范围选；批量删除 / 收藏 / 移分组 / 合并复制 |
| B3 | 编辑条目内容 | 纯文本编辑，Rust 重算 hash / summary / search_text / sub_kind |
| B4 | 粘贴队列 | 选中多条按顺序依次粘贴 |
| B5 | 暂停记录 | 托盘与命令：暂停 N 分钟 / 直到恢复；忽略下一次复制 |
| B6 | 按正则排除内容 | `clipboard.filters.excludedPatterns`，Rust 入库前过滤 |
| B7 | 粘贴转换 | 右键「粘贴为…」：大写 / 小写 / 去首尾空白 / 单行 / JSON 格式化 / URL 编解码 |

## C. Bug 排查

review 过程中发现的 bug 单独修复并记录在 journal。

## Constraints

- 遵守 AGENTS.md：Rust-First、仅 macOS + Windows、已发布 migration 不回改、新设置项 `#[serde(default)]`。
- 每个子项独立提交，Conventional Commits。
