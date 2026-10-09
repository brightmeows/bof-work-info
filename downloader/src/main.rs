use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};

mod sources;

#[derive(Debug, Serialize, Deserialize)]
struct BmsData {
    entries: Vec<sources::BmsEntry>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 事件文件路径 (例如: events/BOFTT.toml)
    #[arg(short, long)]
    event: PathBuf,

    /// 输出目录
    #[arg(short, long, default_value = "downloads")]
    output: PathBuf,

    /// 下载特定作品编号 (例如: 1,2,3)
    #[arg(long)]
    entries: Option<String>,

    /// 交互模式 - 为每个作品选择下载链接
    #[arg(long)]
    interactive: bool,

    /// 日志级别 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

fn load_event_data(path: &Path) -> Result<BmsData> {
    info!("加载事件文件: {}", path.display());
    let content =
        fs::read_to_string(path).with_context(|| format!("无法读取文件: {}", path.display()))?;

    let data: BmsData = toml::from_str(&content)
        .with_context(|| format!("解析TOML文件失败: {}", path.display()))?;

    info!("加载了 {} 个作品", data.entries.len());
    Ok(data)
}

fn filter_entries<'a>(
    data: &'a BmsData,
    entries_filter: Option<&'a str>,
) -> Vec<&'a sources::BmsEntry> {
    if let Some(filter) = entries_filter {
        let numbers: Vec<&str> = filter.split(',').map(str::trim).collect();
        let mut filtered = Vec::new();

        for entry in &data.entries {
            if numbers.contains(&entry.no.as_str()) {
                filtered.push(entry);
            }
        }

        if filtered.is_empty() {
            warn!("没有找到编号为 {filter} 的作品");
        }
        filtered
    } else {
        data.entries.iter().collect()
    }
}

async fn async_main(args: Args) -> Result<()> {
    // 加载事件数据
    let data = load_event_data(&args.event)?;

    // 过滤作品
    let entries = filter_entries(&data, args.entries.as_deref());

    if entries.is_empty() {
        error!("没有找到要下载的作品");
        return Ok(());
    }

    // 创建输出目录
    fs::create_dir_all(&args.output)?;

    info!(
        "开始下载 {} 个作品到 {}",
        entries.len(),
        args.output.display()
    );

    // 下载每个作品
    for entry in entries {
        if let Err(e) = sources::download_entry(entry, &args.output, args.interactive).await {
            error!("下载作品 #{} 失败: {}", entry.no, e);
        }
    }

    info!("下载完成！");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 初始化日志
    env_logger::Builder::from_default_env()
        .filter_level(match args.log_level.as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();

    async_main(args).await
}
