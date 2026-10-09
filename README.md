# bof-work-info

manbow（[manbow.nothing.sh/event](https://manbow.nothing.sh/event/)）BMS 活动数据仓库。自动发现并抓取站上全部活动的作品列表、报名一览（评分统计）与团队档案，以 TOML 文件存档，由 GitHub Actions 每周自动更新。

## 仓库结构

- `events/<事件id>.toml`：每个活动一个数据文件，id 为 manbow 站的事件编号（如 `22.toml` 是 BOF2005，`152.toml` 是进行中的 BOF:22）
- `fetcher/`：抓取工具，负责事件发现、页面解析与数据落盘
- `downloader/`：作品下载工具，从数据文件读取下载链接并抓取作品文件
- `.github/workflows/`：
  - `update-events.yml`：每周五 12:00 UTC 自动更新，有变化的事件各自开 PR 并自动 merge
  - `ci.yml`：构建、clippy（pedantic deny）与 rustfmt 检查
  - `cleanup-merged-pr-branches.yml`：清理已合并的更新分支

## 数据文件格式

```toml
[[entries]]
no = "1"
name = "Aquer"
team = "Yobimo Entertainment"
title = "ALYA"
size = "130700 KB"
addr = ["G-Drive:", "https://drive.google.com/..."]
genre = "Botanical Hi-Tech"   # 以下字段来自报名一览页，无数据时缺省
impr = "0"
total = "0"
median = "0"
avg = "0"
update = "2026/10/09 19:25"

[[teams]]                      # 团队档案，仅进行中的活动提供
no = "1"
team = "Yobimo Entertainment"
leader = "Aquer"
member = "3人"
works = "2 / 3作品"
```

## 工具用法

抓取（详细说明见 `fetcher/README.md`）：

```bash
cargo run --release -p bof-table-fetch                # 全量抓取到 events/
cargo run --release -p bof-table-fetch -- --event 152 # 只抓指定事件
cargo run --release -p bof-table-fetch -- --list-events # 输出事件清单 JSON
```

下载（详细说明见 `downloader/README.md`）：

```bash
cargo run --release -p downloader -- --event events/146.toml
cargo run --release -p downloader -- --event events/146.toml --entries "1,3,5"
```

## 自动更新流程

1. `fetcher --list-events` 从活动列表页发现全部事件 id，生成 CI 矩阵
2. 每个事件一个 job：抓取三种页面写入 `events/<id>.toml`，无变化则跳过
3. 有变化的事件各自创建 PR（`chore/update-<id>` 分支）并启用自动 merge（merge commit 方式）

数据输出是确定性的：相同输入产出字节相同的文件，因此 git diff 只反映真实数据变化。

## 开发

Rust workspace，两个成员 crate。工具链为 stable Rust（edition 2024）。

```bash
cargo build --workspace
cargo clippy --workspace --all-targets   # pedantic 级 lint，deny
cargo fmt --all
```

依赖由 renovate 自动更新；`Cargo.lock` 已提交。
