# BOF Table Downloader

一个用于下载BMS活动作品文件的Rust工具，支持从events/*.toml文件中读取作品信息并下载到本地。

数据文件来自本仓库的 `events/<事件id>.toml`，由 fetcher 自动更新。

## 功能特性

- 🔍 自动识别多种下载链接类型（直链、Google Drive、Dropbox、OneDrive、MediaFire等）
- 📝 支持从events/*.toml文件读取作品信息
- 🎯 支持按作品编号筛选下载
- 🤝 交互模式支持多链接选择
- 📁 自动创建输出目录和清理文件名
- 🐛 完整的日志系统
- ✅ **只接受完整的URL格式** - 不再支持纯分享ID

## 支持的链接类型

> **重要**: 只接受完整的URL格式，不再支持纯分享ID

- **直链**: 直接HTTP/HTTPS下载链接
- **Google Drive**: 
  - 支持完整URL格式：`https://drive.google.com/file/d/ID/view?usp=sharing`
  - 支持UC格式：`https://drive.google.com/uc?export=download&id=ID`
  - 支持UserContent格式：`https://drive.usercontent.google.com/download?id=ID`
  - 支持UserContent UC格式：`https://drive.usercontent.google.com/u/0/uc?id=ID&export=download`
  - 支持Drive Link格式：`https://drive.google.com/file/d/ID/view?usp=drive_link`
  - 自动从完整URL中提取分享ID并转换为直接下载链接
- **Dropbox**: 
  - 支持完整URL格式：`https://www.dropbox.com/s/ID/filename`
  - 支持SCL FI格式：`https://www.dropbox.com/scl/fi/ID/filename?rlkey=xxx`
  - 支持SCL FO格式：`https://www.dropbox.com/scl/fo/ID/filename?rlkey=xxx`
  - 支持Dropboxusercontent格式：`https://dl.dropboxusercontent.com/scl/fi/ID/filename?rlkey=xxx`
  - 自动从完整URL中提取分享ID并转换为直接下载链接
- **OneDrive**: 支持1drv.ms短链接
- **MediaFire**: 支持mediafire.com链接
- **Mega**: 识别但不支持下载（需要特殊处理）

## 安装

```bash
cargo build --release
```

## 使用方法

### 基本用法

```bash
# 下载指定事件的所有作品
cargo run -p downloader -- --event events/146.toml

# 下载到指定目录
cargo run -p downloader -- --event events/146.toml --output my_downloads

# 下载特定作品编号
cargo run -p downloader -- --event events/146.toml --entries "1,3,5"
```

### 交互模式

当作品有多个下载链接时，使用交互模式进行选择：

```bash
cargo run -p downloader -- --event events/146.toml --interactive
```

### 命令行参数

- `-e, --event <PATH>`: 事件文件路径（必需）
- `-o, --output <DIR>`: 输出目录，默认为 `downloads`
- `--entries <NUMBERS>`: 要下载的作品编号，用逗号分隔（例如：1,3,5）
- `--interactive`: 交互模式，为每个作品选择下载链接
- `--log-level <LEVEL>`: 日志级别 (trace, debug, info, warn, error)，默认为info

## 使用示例

### 下载指定活动的所有作品

```bash
cargo run -p downloader -- --event events/146.toml --output boftt_downloads  # 参数名自定
```

### 下载特定作品

```bash
# 下载作品编号1, 5, 10
cargo run -p downloader -- --event events/146.toml --entries "1,5,10"
```

### 交互模式选择下载链接

```bash
cargo run -p downloader -- --event events/146.toml --interactive
```

输出示例：
```
作品 #1 - Jour Intense
作者: Clara Montclair
团队: Cynical★4
大小: 16328 KB

可用的下载链接:
  1. Dropbox("xv5y8nncofb9yeh3h9brc") -> https://www.dropbox.com/s/xv5y8nncofb9yeh3h9brc/file?dl=1

请选择要下载的链接 (输入数字，或按 Enter 跳过):
```

### 链接格式示例

在events/*.toml文件中，**只接受完整的URL格式**：

```toml
[[entries]]
no = "1"
name = "作者名"
title = "作品标题"
size = "1024 KB"
addr = [
    # Google Drive - 支持多种格式
    "https://drive.google.com/file/d/1jcN3IRYuRcLaact9vHhU1zNzEUdggAtD/view?usp=sharing",
    "https://drive.google.com/uc?export=download&id=1jcN3IRYuRcLaact9vHhU1zNzEUdggAtD",
    "https://drive.usercontent.google.com/download?id=1m_GOnfpSIH-ZLMRBQnQT_avgCrSywIko",
    "https://drive.usercontent.google.com/u/0/uc?id=1NH6o6K87t8SyALD4dnlhoWjfmuhlhUJ1&export=download",
    "https://drive.google.com/file/d/1imaDyb6IVyLghtU9LurhI_kkwPZ5yOLd/view?usp=drive_link",
    
    # Dropbox - 支持多种格式
    "https://www.dropbox.com/s/xv5y8nncofb9yeh3h9brc/filename.zip",
    "https://www.dropbox.com/scl/fi/xv5y8nncofb9yeh3h9brc/filename.zip?rlkey=xxx",
    "https://www.dropbox.com/scl/fo/18srn7s8rj8voez0f5v8p/filename.zip?rlkey=xxx",
    "https://dl.dropboxusercontent.com/scl/fi/dyqxgoa9y0bae5l2aspmc/filename.zip?rlkey=xxx",
    
    # 其他链接
    "https://example.com/file.zip"        # 直链
]
```

> **注意**: 不再支持纯分享ID格式（如 `1jcN3IRYuRcLaact9vHhU1zNzEUdggAtD`），必须使用完整的URL。

### 链接提取功能

downloader会自动从完整的分享链接中提取分享ID并转换为直接下载链接：

- **Google Drive**: 
  - 从 `/file/d/ID/view` 格式中提取ID并转换为UC下载链接
  - 从 `?id=ID` 参数中提取ID
  - 从 `/download?id=ID` 格式中提取ID
  - 从 `/uc?id=ID` 格式中提取ID
- **Dropbox**: 从 `/s/ID/filename`、`/scl/fi/ID/filename`、`/scl/fo/ID/filename` 或 `dl.dropboxusercontent.com/scl/fi/ID/filename` 格式中提取ID并转换为直接下载链接
- 提取的ID用于构造更简洁的直接下载链接

## 文件命名规则

下载的文件将按以下格式命名：
```
{作品编号} - {作品标题}
```

例如：
- `1 - Jour Intense`
- `2 - カメさんレースを……邪魔するなああああああ！！！`

文件名中的非法字符会被自动替换为下划线。

## 链接处理逻辑

### 单链接情况
- 如果作品只有一个可下载链接，直接下载
- 如果链接类型不支持，显示警告并跳过

### 多链接情况
- **非交互模式**: 显示所有可用链接，提示使用 `--interactive` 模式
- **交互模式**: 显示所有链接供用户选择

### 无链接情况
- 显示警告信息
- 如果有非链接内容（如说明文字），也会显示

## 日志级别

- `trace`: 最详细的日志，包括所有内部操作
- `debug`: 调试信息，包括链接解析过程
- `info`: 一般信息，包括下载进度
- `warn`: 警告信息，如不支持的链接类型
- `error`: 错误信息，如下载失败

## 错误处理

- 网络错误：自动重试（如果支持）
- 文件系统错误：创建目录失败等
- 链接解析错误：不支持的链接格式
- 用户输入错误：无效的作品编号等

## 依赖项

- `surf`: HTTP客户端
- `smol`: 异步运行时
- `clap`: 命令行参数解析
- `toml`: TOML格式支持
- `serde`: 序列化框架
- `anyhow`: 错误处理
- `log` + `env_logger`: 日志系统
- `url`: URL解析
- `regex`: 正则表达式
- `indicatif`: 进度条显示
- `infer`: 文件类型检测
