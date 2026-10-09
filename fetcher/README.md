# fetcher

manbow BMS 活动数据抓取工具：从 manbow 站点自动发现全部活动，抓取每个活动的作品列表、报名一览（评分统计）与团队档案，输出为 TOML。

## 数据来源

每个活动抓取三种页面：

- `event.cgi?action=URLList&end=999&event=<id>`：作品列表（作者、团队、标题、大小、下载链接），必抓
- `event.cgi?action=sp&event=<id>`：报名一览，补充 genre、impr、total、total_n、median、avg、regist、update 评分字段；抓不到时降级跳过
- `event_teamprofile.cgi?event=<id>`：团队档案，写入 `[[teams]]` 段；仅进行中的活动提供，无效时降级跳过

页面解析按表头断言：表头单元格必须全部落在该页面类型的已知列名集合内，否则报错，不猜测列位置。HTML 解码优先采用页面 meta charset 声明，随后依次尝试严格 UTF-8、Shift_JIS（Windows-31J）、EUC-JP 解码。

## 使用

```bash
# 全量：发现全部事件并抓取到 events/ 目录（文件名为裸事件 id）
cargo run --release

# 只抓指定事件（可重复）
cargo run --release -- --event 22 --event 152

# 输出事件清单（JSON 数组），供 CI 生成矩阵
cargo run --release -- --list-events

# 调试：抓取指定 URL 并合并输出到 stdout
echo "https://manbow.nothing.sh/event/event.cgi?action=URLList&end=999&event=146" \
  | cargo run --release -- --stdin

# 请求间隔毫秒数与输出目录
cargo run --release -- --dir events --delay-ms 500
```

全部参数见 `cargo run -- --help`。

## 输出格式

每个活动一个文件（`events/<事件id>.toml`）：

```toml
[[entries]]
no = "1"
name = "Aquer"
team = "Yobimo Entertainment"
title = "ALYA"
size = "130700 KB"
addr = ["G-Drive:", "https://drive.google.com/...", "..."]
genre = "Botanical Hi-Tech"
impr = "0"
total = "0"
median = "0"
avg = "0"
update = "2026/10/09 19:25"

[[teams]]
no = "1"
team = "Yobimo Entertainment"
leader = "Aquer"
member = "3人"
works = "2 / 3作品"
```

无评分数据或无团队档案时对应字段与段落缺省。同一次运行的输出是确定性的：相同输入产出字节相同的结果，CI 依赖这一点以 git diff 判定是否有变化。

## 开发

```bash
cargo clippy --workspace --all-targets   # pedantic 级 lint，deny
cargo fmt --all
```
