//! manbow BMS 活动数据抓取工具。
//!
//! 默认模式：从事件列表页自动发现全部事件，逐事件抓取作品列表、
//! 报名一览与团队档案，写入 events/<事件id>.toml。
//! stdin 模式（--stdin）：抓取手动指定的页面 URL 并合并输出，供调试。

use std::{
    io::{self, Read},
    path::Path,
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result};
use clap::Parser;
use log::{error, info, warn};

mod manbow;
mod model;

use model::{BmsData, DetailsData, ManbowEvent, TeamEntry};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// 输出目录（默认模式）
    #[arg(short, long, default_value = "events")]
    dir: PathBuf,

    /// 只处理指定事件 id（可重复；默认全部）
    #[arg(short, long = "event")]
    event_ids: Vec<String>,

    /// 强制抓取作品详情页的事件 id（可重复；未指定时按团队页判定）
    #[arg(long = "details")]
    details_ids: Vec<String>,

    /// 列出站点上发现的全部事件（JSON）后退出
    #[arg(long, conflicts_with = "stdin")]
    list_events: bool,

    /// 从 stdin 读取 URL 列表（每行一个，调试用）
    #[arg(long)]
    stdin: bool,

    /// stdin 模式的输出文件（默认 stdout）
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// 请求间隔毫秒数
    #[arg(long, default_value_t = 500)]
    delay_ms: u64,

    /// 日志级别 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::Builder::from_default_env()
        .filter_level(match args.log_level.as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();

    if args.stdin {
        run_stdin(&args).await
    } else {
        run_events(&args).await
    }
}

async fn fetch_with_delay(url: &str, delay: Duration) -> Result<String> {
    tokio::time::sleep(delay).await;
    manbow::fetch_text(url).await
}

async fn discover_events(args: &Args) -> Result<Vec<ManbowEvent>> {
    info!("抓取事件列表: {}", manbow::EVENT_LIST_URL);
    let html = fetch_with_delay(manbow::EVENT_LIST_URL, delay_of(args)).await?;
    let events = manbow::discover_events(&html);
    info!("发现 {} 个事件", events.len());
    Ok(events)
}

fn delay_of(args: &Args) -> Duration {
    Duration::from_millis(args.delay_ms)
}

async fn run_events(args: &Args) -> Result<()> {
    if args.list_events {
        let events = discover_events(args).await?;
        println!("{}", serde_json::to_string(&events)?);
        return Ok(());
    }

    let delay = delay_of(args);
    let mut events = discover_events(args).await?;
    if !args.event_ids.is_empty() {
        events.retain(|event| args.event_ids.contains(&event.id));
    }
    if events.is_empty() {
        error!("没有可处理的事件");
        return Ok(());
    }

    std::fs::create_dir_all(&args.dir)
        .with_context(|| format!("创建输出目录失败: {}", args.dir.display()))?;

    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    for event in &events {
        match fetch_event(event, delay, &args.details_ids).await {
            Ok(event_data) => {
                let data = event_data.data;
                let path = args.dir.join(format!("{}.toml", event.id));
                let content = toml::to_string_pretty(&data)?;
                std::fs::write(&path, content)
                    .with_context(|| format!("写入失败: {}", path.display()))?;
                info!(
                    "[{}] {} 条作品 {} 个团队 -> {}",
                    event.id,
                    data.entries.len(),
                    data.teams.len(),
                    path.display()
                );
                if let Some(details) = event_data.details {
                    let path = args.dir.join(format!("{}.details.toml", event.id));
                    let content = toml::to_string_pretty(&details)?;
                    std::fs::write(&path, content)
                        .with_context(|| format!("写入失败: {}", path.display()))?;
                    info!(
                        "[{}] {} 条作品详情 -> {}",
                        event.id,
                        details.details.len(),
                        path.display()
                    );
                }
                succeeded += 1;
            }
            Err(e) => {
                error!("[{}] 抓取失败: {e:#}", event.id);
                failed += 1;
            }
        }
    }

    info!("完成: {succeeded} 成功, {failed} 失败");
    Ok(())
}

/// 单事件抓取结果；详情仅在活动进行中时产出。
struct EventData {
    data: BmsData,
    details: Option<DetailsData>,
}

/// 抓取单个事件的页面；报名一览与团队档案失败时降级跳过。
///
/// 团队列表解析成功即视为进行中活动，进而抓取每作品详情页（`More_def`）
/// 与团队详情子页；历史活动无这些页面，自动跳过。
async fn fetch_event(
    event: &ManbowEvent,
    delay: Duration,
    details_ids: &[String],
) -> Result<EventData> {
    let mut data = {
        let html = fetch_with_delay(&manbow::urllist_url(&event.id), delay).await?;
        manbow::parse_urllist(&html)?
    };

    match fetch_with_delay(&manbow::sp_url(&event.id), delay)
        .await
        .and_then(|html| manbow::parse_sp_entries(&html))
    {
        Ok(extra) => {
            data.entries = manbow::merge_entries(std::mem::take(&mut data.entries), extra);
        }
        Err(e) => warn!("[{}] 报名一览不可用: {e:#}", event.id),
    }

    let mut details: Option<Vec<model::EntryDetail>> = None;
    match fetch_with_delay(&manbow::team_profile_url(&event.id), delay)
        .await
        .and_then(|html| manbow::parse_team_entries(&html))
    {
        Ok(teams) => {
            let active = !teams.is_empty() || details_ids.contains(&event.id);
            data.teams = enrich_teams(&event.id, teams, delay).await;
            if active {
                details = Some(fetch_details(&event.id, &data.entries, delay).await);
            }
        }
        Err(e) => {
            info!("[{}] 团队档案不可用: {e:#}", event.id);
            if details_ids.contains(&event.id) {
                details = Some(fetch_details(&event.id, &data.entries, delay).await);
            }
        }
    }

    Ok(EventData {
        data,
        details: details.map(|details| DetailsData { details }),
    })
}

/// 逐团队抓取详情子页补全字段；单个子页失败降级为仅列表数据。
async fn enrich_teams(event_id: &str, teams: Vec<TeamEntry>, delay: Duration) -> Vec<TeamEntry> {
    let mut enriched = Vec::with_capacity(teams.len());
    for team in teams {
        let Some(team_id) = team.team_id.clone() else {
            enriched.push(team);
            continue;
        };
        let url = manbow::team_profile_sub_url(event_id, &team_id);
        match fetch_with_delay(&url, delay).await {
            Ok(html) if html.contains("Team Profile") => {
                enriched.push(manbow::parse_team_sub(&html, team));
            }
            Ok(_) => {
                warn!("[{event_id}] 团队 {team_id} 详情子页不可用（回退页）");
                enriched.push(team);
            }
            Err(e) => {
                warn!("[{event_id}] 团队 {team_id} 详情子页抓取失败: {e:#}");
                enriched.push(team);
            }
        }
    }
    enriched
}

/// 逐作品抓取 `More_def` 详情页；单个页面失败降级为缺该条详情。
async fn fetch_details(
    event_id: &str,
    entries: &[model::BmsEntry],
    delay: Duration,
) -> Vec<model::EntryDetail> {
    let mut details = Vec::with_capacity(entries.len());
    for entry in entries {
        let url = manbow::more_def_url(event_id, &entry.no);
        match fetch_with_delay(&url, delay)
            .await
            .and_then(|html| manbow::parse_more_def(&html, &entry.no))
        {
            Ok(detail) => details.push(detail),
            Err(e) => warn!("[{event_id}] 作品 {} 详情页不可用: {e:#}", entry.no),
        }
    }
    details
}

async fn run_stdin(args: &Args) -> Result<()> {
    let delay = delay_of(args);
    let urls = read_urls_from_stdin()?;
    if urls.is_empty() {
        error!("没有输入 URL");
        return Ok(());
    }

    let mut base: Option<Vec<model::BmsEntry>> = None;
    let mut sp_entries: Vec<model::BmsEntry> = Vec::new();
    let mut teams: Vec<TeamEntry> = Vec::new();
    let mut details_data: Option<DetailsData> = None;

    for url in &urls {
        let kind = manbow::classify_url(url);
        info!("处理 {kind:?}: {url}");
        let html = match fetch_with_delay(url, delay).await {
            Ok(html) => html,
            Err(e) => {
                error!("抓取失败 {url}: {e:#}");
                continue;
            }
        };

        let parse_result = match kind {
            manbow::UrlKind::UrlList => manbow::parse_urllist(&html).map(|data| {
                base = Some(data.entries);
            }),
            manbow::UrlKind::Sp => manbow::parse_sp_entries(&html).map(|entries| {
                sp_entries = entries;
            }),
            manbow::UrlKind::TeamProfile => manbow::parse_team_entries(&html).map(|parsed| {
                teams = parsed;
            }),
            manbow::UrlKind::TeamProfileSub => {
                teams = vec![manbow::parse_team_sub(&html, TeamEntry::stub())];
                Ok(())
            }
            manbow::UrlKind::MoreDef => manbow::parse_more_def(&html, "1").map(|detail| {
                details_data = Some(DetailsData {
                    details: vec![detail],
                });
            }),
            manbow::UrlKind::Unknown => {
                warn!("无法识别的 URL，跳过: {url}");
                continue;
            }
        };
        if let Err(e) = parse_result {
            error!("解析失败 {url}: {e:#}");
        }
    }

    // MoreDef 页单独输出详情；其余页面类型合并输出 BmsData
    if let Some(details) = details_data {
        let output = toml::to_string_pretty(&details)?;
        write_output(&output, args.output.as_deref())?;
        return Ok(());
    }

    let mut data = BmsData {
        entries: base.unwrap_or_default(),
        teams,
    };
    data.entries = manbow::merge_entries(std::mem::take(&mut data.entries), sp_entries);

    let output = toml::to_string_pretty(&data)?;
    write_output(&output, args.output.as_deref())?;
    Ok(())
}

fn read_urls_from_stdin() -> Result<Vec<String>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let urls: Vec<String> = input
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| line.starts_with("http"))
        .collect();
    info!("从 stdin 读取到 {} 个 URL", urls.len());
    Ok(urls)
}

fn write_output(content: &str, output_path: Option<&Path>) -> Result<()> {
    if let Some(path) = output_path {
        std::fs::write(path, content).with_context(|| format!("写入失败: {}", path.display()))?;
        info!("数据已保存到文件: {}", path.display());
    } else {
        print!("{content}");
    }
    Ok(())
}
