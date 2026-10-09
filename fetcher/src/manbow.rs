//! manbow 站点的抓取与页面解析。
//!
//! 三种页面均为固定结构的表格，解析采用表头断言：表头单元格必须全部落在
//! 该页面类型的已知列名集合内，否则视为页面回退或改版而报错，不猜列位。

use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use anyhow::{Context, Result, bail};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::model::{BmsData, BmsEntry, ManbowEvent, TeamEntry};

pub(crate) const EVENT_LIST_URL: &str = "https://manbow.nothing.sh/event/event.cgi";

const BASE_URL: &str = "https://manbow.nothing.sh/event";

/// stdin 模式下 URL 的页面类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UrlKind {
    UrlList,
    Sp,
    TeamProfile,
    Unknown,
}

pub(crate) fn classify_url(url: &str) -> UrlKind {
    if url.contains("event_teamprofile.cgi") {
        UrlKind::TeamProfile
    } else if url.contains("action=sp") {
        UrlKind::Sp
    } else if url.contains("action=URLList") {
        UrlKind::UrlList
    } else {
        UrlKind::Unknown
    }
}

pub(crate) fn urllist_url(event_id: &str) -> String {
    format!("{BASE_URL}/event.cgi?action=URLList&end=999&event={event_id}")
}

pub(crate) fn sp_url(event_id: &str) -> String {
    format!("{BASE_URL}/event.cgi?action=sp&event={event_id}")
}

pub(crate) fn team_profile_url(event_id: &str) -> String {
    format!("{BASE_URL}/event_teamprofile.cgi?event={event_id}")
}

/// 抓取 URL 并按页面自声明编码解码为文本。
pub(crate) async fn fetch_text(url: &str) -> Result<String> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("请求失败: {url}"))?;

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("读取响应失败: {url}"))?;

    if !status.is_success() {
        bail!("HTTP {status}: {url}");
    }

    Ok(decode_html(&bytes, content_type.as_deref()))
}

/// 按页面自声明的编码解码字节流。
///
/// 依次尝试：meta charset、HTTP Content-Type charset 参数、UTF-8、Shift_JIS、
/// EUC-JP（后三者严格解码），全部失败则按 UTF-8 替换非法字节。
/// 站点实际为 Windows-31J（WHATWG `shift_jis` 标签），meta 声明命中时直接采用。
pub(crate) fn decode_html(bytes: &[u8], content_type: Option<&str>) -> String {
    let mut labels: Vec<String> = Vec::new();
    if let Some(label) = sniff_meta_charset(bytes) {
        labels.push(label);
    }
    if let Some(label) = content_type.and_then(header_charset) {
        labels.push(label);
    }
    labels.extend(["utf-8", "shift_jis", "euc-jp"].map(str::to_string));

    let mut tried = HashSet::new();
    for label in &labels {
        if !tried.insert(label.as_str()) {
            continue;
        }
        if let Some(text) = decode_strict(label, bytes) {
            return text;
        }
    }

    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_strict(label: &str, bytes: &[u8]) -> Option<String> {
    let encoding = encoding_rs::Encoding::for_label(label.as_bytes())?;
    let (text, _, had_errors) = encoding.decode(bytes);
    (!had_errors).then(|| text.into_owned())
}

/// 从文档前部嗅探 meta charset 声明。
fn sniff_meta_charset(bytes: &[u8]) -> Option<String> {
    static META_CHARSET: OnceLock<Regex> = OnceLock::new();
    let meta_charset = META_CHARSET.get_or_init(|| {
        Regex::new(r#"(?i)<meta[^>]+charset\s*=\s*["']?\s*([\w-]+)"#).expect("静态正则必然编译成功")
    });

    let head = &bytes[..bytes.len().min(2048)];
    let text = String::from_utf8_lossy(head);
    meta_charset
        .captures(&text)?
        .get(1)
        .map(|m| m.as_str().to_ascii_lowercase())
}

fn header_charset(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        part.trim()
            .strip_prefix("charset=")
            .map(|label| label.trim_matches('"').to_ascii_lowercase())
    })
}

/// 从事件列表页发现全部事件，按 id 数值升序返回。
pub(crate) fn discover_events(html: &str) -> Vec<ManbowEvent> {
    let document = Html::parse_document(html);
    let anchor = Selector::parse("a").expect("静态选择器必然解析成功");

    // id -> 标题候选；同一事件的多个入口链接（List_def/Details/sp 等）取信息量最大的一条
    let mut titles: HashMap<String, Option<String>> = HashMap::new();
    for element in document.select(&anchor) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        let Some(id) = extract_event_id(href) else {
            continue;
        };
        let text = clean_cell_text(element);
        let slot = titles.entry(id).or_default();
        if text.is_empty() || text.starts_with('[') {
            continue; // [D]/[L] 类入口链接，仅作标题候选排除
        }
        if slot
            .as_ref()
            .is_none_or(|existing| text.len() > existing.len())
        {
            *slot = Some(text);
        }
    }

    let mut events: Vec<ManbowEvent> = titles
        .into_iter()
        .filter_map(|(id, title)| id.parse::<u32>().ok().map(|_| ManbowEvent { id, title }))
        .collect();
    events.sort_by_key(|event| event.id.parse::<u32>().expect("已过滤为数字 id"));
    events
}

/// 从链接地址提取事件 id。
fn extract_event_id(href: &str) -> Option<String> {
    static EVENT_ID: OnceLock<Regex> = OnceLock::new();
    let event_id =
        EVENT_ID.get_or_init(|| Regex::new(r"[?&]event=(\d+)").expect("静态正则必然编译成功"));

    if !href.contains("event") {
        return None;
    }
    event_id
        .captures(href)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string())
}

/// 解析后的表格：列名（已规范化）与数据行。
struct ParsedTable {
    cols: Vec<String>,
    rows: Vec<TableRow>,
}

struct TableRow {
    texts: Vec<String>,
    htmls: Vec<String>,
}

/// 提取主表：找到与已知列名集合匹配的表头行，其后的行为数据行。
///
/// 断言规则：表头单元格数不少于 3、无重复列名、且每个列名（规范化后）
/// 都落在 expected 集合内。不满足即报错——manbow 页面回退到事件列表等
/// 异常内容会在此被拦截，而不是静默产出错误数据。
fn extract_table(html: &str, expected: &[&str], label: &str) -> Result<ParsedTable> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("tr").expect("静态选择器必然解析成功");
    let cell_selector = Selector::parse("td, th").expect("静态选择器必然解析成功");

    let mut cols: Option<Vec<String>> = None;
    let mut rows = Vec::new();

    for row in document.select(&row_selector) {
        let cells: Vec<ElementRef> = row.select(&cell_selector).collect();
        if cells.is_empty() {
            continue;
        }
        let texts: Vec<String> = cells.iter().map(|cell| clean_cell_text(*cell)).collect();

        if let Some(found) = &cols {
            if texts.len() != found.len() {
                continue; // 跨列等非数据行
            }
            let htmls = cells.iter().map(ElementRef::inner_html).collect();
            rows.push(TableRow { texts, htmls });
        } else {
            let normalized: Vec<String> = texts.iter().map(|text| normalize_header(text)).collect();
            if is_known_header(&normalized, expected) {
                cols = Some(normalized);
            }
        }
    }

    let Some(cols) = cols else {
        bail!("{label}: 未找到匹配的表头行（已知列名: {expected:?}）");
    };
    Ok(ParsedTable { cols, rows })
}

fn normalize_header(text: &str) -> String {
    text.trim().trim_end_matches('.').to_lowercase()
}

fn is_known_header(normalized: &[String], expected: &[&str]) -> bool {
    normalized.len() >= 3
        && normalized
            .iter()
            .all(|name| expected.contains(&name.as_str()))
        && normalized.iter().collect::<HashSet<_>>().len() == normalized.len()
}

impl ParsedTable {
    fn col(&self, name: &str) -> Option<usize> {
        self.cols.iter().position(|col| col == name)
    }

    fn require_col(&self, name: &str, label: &str) -> Result<usize> {
        self.col(name)
            .ok_or_else(|| anyhow::anyhow!("{label}: 缺少必需列 {name}"))
    }
}

/// 读取行内指定列的文本，越界或缺列时为空串。
fn row_text(row: &TableRow, idx: Option<usize>) -> String {
    match idx {
        Some(i) if i < row.texts.len() => row.texts[i].clone(),
        _ => String::new(),
    }
}

/// 同 [`row_text`]，空串归一化为 `None`。
fn row_optional_text(row: &TableRow, idx: Option<usize>) -> Option<String> {
    let text = row_text(row, idx);
    (!text.is_empty()).then_some(text)
}

/// 解析作品链接列表页（event.cgi?action=URLList）。
pub(crate) fn parse_urllist(html: &str) -> Result<BmsData> {
    const KNOWN: [&str; 6] = ["no", "name", "team", "title", "size", "addr"];
    let label = "URLList";
    let table = extract_table(html, &KNOWN, label)?;

    let i_no = table.require_col("no", label)?;
    let i_name = table.require_col("name", label)?;
    let i_title = table.require_col("title", label)?;
    let i_addr = table.require_col("addr", label)?;
    let i_team = table.col("team");
    let i_size = table.col("size");

    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for row in &table.rows {
        let no = cell_no(row, i_no);
        let name = row_text(row, Some(i_name));
        let title = row_text(row, Some(i_title));

        let key = format!("{no}|{name}|{title}");
        if !seen.insert(key) {
            continue;
        }

        let team = row_optional_text(row, i_team);
        let size = row_text(row, i_size);
        let addr_html = row.htmls.get(i_addr).cloned().unwrap_or_default();
        let addr: Vec<String> = addr_html
            .split("<br>")
            .flat_map(extract_urls_and_text)
            .filter(|item| !item.trim().is_empty())
            .collect();

        entries.push(BmsEntry {
            no,
            name,
            team,
            title,
            size,
            addr,
            genre: None,
            impr: None,
            total: None,
            total_n: None,
            median: None,
            avg: None,
            regist: None,
            update: None,
        });
    }

    Ok(BmsData {
        entries,
        teams: Vec::new(),
    })
}

/// 提取序号：优先取纯数字，否则从单元格 HTML 中抓数字。
fn cell_no(row: &TableRow, i_no: usize) -> String {
    static NUM: OnceLock<Regex> = OnceLock::new();
    let num = NUM.get_or_init(|| Regex::new(r"\d+").expect("静态正则必然编译成功"));

    let text = row_text(row, Some(i_no));
    if text.parse::<u32>().is_ok() {
        return text;
    }

    let html = row.htmls.get(i_no).map(String::as_str).unwrap_or_default();
    num.find(html)
        .map(|m| m.as_str().to_string())
        .unwrap_or(text)
}

/// 解析报名一览页（event.cgi?action=sp），返回带评分统计的条目。
///
/// 不同年代的页面列数不同（如 Total(N) 列为后来新增），以表头为准取列。
pub(crate) fn parse_sp_entries(html: &str) -> Result<Vec<BmsEntry>> {
    const KNOWN: [&str; 12] = [
        "no", "team", "artist", "genre", "title", "impr", "total", "total(n)", "median", "avg",
        "regist", "update",
    ];
    let label = "sp";
    let table = extract_table(html, &KNOWN, label)?;

    let i_no = table.require_col("no", label)?;
    let i_artist = table.require_col("artist", label)?;
    let i_title = table.require_col("title", label)?;
    let i_team = table.col("team");
    let i_genre = table.col("genre");
    let i_impr = table.col("impr");
    let i_total = table.col("total");
    let i_total_n = table.col("total(n)");
    let i_median = table.col("median");
    let i_avg = table.col("avg");
    let i_regist = table.col("regist");
    let i_update = table.col("update");

    let mut entries = Vec::new();
    for row in &table.rows {
        entries.push(BmsEntry {
            no: row_text(row, Some(i_no)),
            name: row_text(row, Some(i_artist)),
            team: row_optional_text(row, i_team),
            title: row_text(row, Some(i_title)),
            size: String::new(),
            addr: Vec::new(),
            genre: row_optional_text(row, i_genre),
            impr: row_optional_text(row, i_impr),
            total: row_optional_text(row, i_total),
            total_n: row_optional_text(row, i_total_n),
            median: row_optional_text(row, i_median),
            avg: row_optional_text(row, i_avg),
            regist: row_optional_text(row, i_regist),
            update: row_optional_text(row, i_update),
        });
    }

    Ok(entries)
}

/// 解析团队档案页（`event_teamprofile.cgi`）。
///
/// 无效事件该地址会回退到事件列表页，表头断言在此拦截并报错。
pub(crate) fn parse_team_entries(html: &str) -> Result<Vec<TeamEntry>> {
    const KNOWN: [&str; 10] = [
        "no",
        "eb",
        "banner",
        "team",
        "leader",
        "member",
        "works",
        "memberlist",
        "regist",
        "update",
    ];
    let label = "teamprofile";
    let table = extract_table(html, &KNOWN, label)?;

    let i_no = table.require_col("no", label)?;
    let i_team = table.require_col("team", label)?;
    let i_leader = table.require_col("leader", label)?;
    let i_member = table.col("member");
    let i_works = table.col("works");
    let i_regist = table.col("regist");
    let i_update = table.col("update");

    let mut teams = Vec::new();
    for row in &table.rows {
        let team = row_text(row, Some(i_team));
        if team.is_empty() {
            continue;
        }
        teams.push(TeamEntry {
            no: row_text(row, Some(i_no)),
            team,
            leader: row_text(row, Some(i_leader)),
            member: row_text(row, i_member),
            works: row_text(row, i_works),
            regist: row_optional_text(row, i_regist),
            update: row_optional_text(row, i_update),
        });
    }

    Ok(teams)
}

/// 以 `no` 为键把 sp 条目的评分统计合并进 `URLList` 条目。
///
/// `URLList` 是 `no/name/team/title/size/addr` 的权威来源；sp 独有的条目
/// （有评分无下载链接）原样追加，`size` 与 `addr` 为空。
pub(crate) fn merge_entries(base: Vec<BmsEntry>, extra: Vec<BmsEntry>) -> Vec<BmsEntry> {
    let mut index: HashMap<String, usize> = base
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.no.clone(), i))
        .collect();
    let mut merged = base;

    for sp in extra {
        if let Some(&i) = index.get(&sp.no) {
            let entry = &mut merged[i];
            if entry.genre.is_none() {
                entry.genre = sp.genre;
            }
            if entry.impr.is_none() {
                entry.impr = sp.impr;
            }
            if entry.total.is_none() {
                entry.total = sp.total;
            }
            if entry.total_n.is_none() {
                entry.total_n = sp.total_n;
            }
            if entry.median.is_none() {
                entry.median = sp.median;
            }
            if entry.avg.is_none() {
                entry.avg = sp.avg;
            }
            if entry.regist.is_none() {
                entry.regist = sp.regist;
            }
            if entry.update.is_none() {
                entry.update = sp.update;
            }
            if entry.team.is_none() {
                entry.team = sp.team;
            }
        } else {
            index.insert(sp.no.clone(), merged.len());
            merged.push(sp);
        }
    }

    merged
}

/// 读取单元格的纯文本（合并子元素文本并修剪空白）。
fn clean_cell_text(element: ElementRef) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
}

/// 清理 HTML 实体并剥掉标签，得到可检索的纯文本。
fn clean_html_content(html_content: &str) -> String {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]*>").expect("静态正则必然编译成功"));

    let content = html_content
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    tag.replace_all(&content, "").trim().to_string()
}

/// 把单元格 HTML 拆分为文本段与 URL 列表（保持出现顺序）。
fn extract_urls_and_text(input: &str) -> Vec<String> {
    static URL: OnceLock<Regex> = OnceLock::new();
    let url = URL.get_or_init(|| {
        // RFC 3986 允许出现在 URL 中的字符
        Regex::new(r"https?://[a-zA-Z0-9\-._~':/?#=&%!+]+").expect("静态正则必然编译成功")
    });

    let cleaned_input = clean_html_content(input);

    let mut result = Vec::new();
    let mut last_end = 0;

    for matched in url.find_iter(&cleaned_input) {
        if matched.start() > last_end {
            let text = cleaned_input[last_end..matched.start()].trim();
            if !text.is_empty() {
                result.push(text.to_string());
            }
        }
        result.push(matched.as_str().to_string());
        last_end = matched.end();
    }

    if last_end < cleaned_input.len() {
        let text = cleaned_input[last_end..].trim();
        if !text.is_empty() {
            result.push(text.to_string());
        }
    }

    if result.is_empty() && !cleaned_input.trim().is_empty() {
        result.push(cleaned_input);
    }

    result
}
