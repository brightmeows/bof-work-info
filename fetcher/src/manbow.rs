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

use crate::model::{BmsData, BmsEntry, EntryDetail, ManbowEvent, Revision, TeamEntry, TeamMember};

pub(crate) const EVENT_LIST_URL: &str = "https://manbow.nothing.sh/event/event.cgi";

const BASE_URL: &str = "https://manbow.nothing.sh/event";

/// stdin 模式下 URL 的页面类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UrlKind {
    UrlList,
    Sp,
    TeamProfile,
    TeamProfileSub,
    MoreDef,
    Unknown,
}

pub(crate) fn classify_url(url: &str) -> UrlKind {
    if url.contains("event_teamprofile.cgi") {
        if extract_query_param(url, "team").is_some() {
            UrlKind::TeamProfileSub
        } else {
            UrlKind::TeamProfile
        }
    } else if url.contains("action=More_def") {
        UrlKind::MoreDef
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

pub(crate) fn more_def_url(event_id: &str, num: &str) -> String {
    format!("{BASE_URL}/event.cgi?action=More_def&num={num}&event={event_id}")
}

pub(crate) fn team_profile_sub_url(event_id: &str, team_id: &str) -> String {
    format!("{BASE_URL}/event_teamprofile.cgi?event={event_id}&team={team_id}")
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
    let i_eb = table.col("eb");
    let i_banner = table.col("banner");
    let i_memberlist = table.col("memberlist");

    let mut teams = Vec::new();
    for row in &table.rows {
        let team = row_text(row, Some(i_team));
        if team.is_empty() {
            continue;
        }
        // no 列链接里的稳定团队 id；徽章/横幅图片链接
        let no_html = row.htmls.get(i_no).cloned().unwrap_or_default();
        let team_id = extract_query_param(&no_html, "team");
        let banner_html = i_banner
            .and_then(|i| row.htmls.get(i).cloned())
            .or_else(|| i_eb.and_then(|i| row.htmls.get(i).cloned()))
            .unwrap_or_default();
        let banner = extract_img_src(&banner_html).map(|src| absolutize_url(&src));
        teams.push(TeamEntry {
            no: row_text(row, Some(i_no)),
            team,
            leader: row_text(row, Some(i_leader)),
            member: row_text(row, i_member),
            works: row_text(row, i_works),
            regist: row_optional_text(row, i_regist),
            update: row_optional_text(row, i_update),
            team_id,
            banner,
            members: row_optional_text(row, i_memberlist),
            leader_country: None,
            ratio_points: Vec::new(),
            final_striker: None,
            team_genre: None,
            team_common: None,
            team_reason: None,
            member_rows: Vec::new(),
        });
    }

    Ok(teams)
}

/// 从 HTML 片段中提取 query 参数值。
fn extract_query_param(html: &str, key: &str) -> Option<String> {
    static PARAM: OnceLock<Regex> = OnceLock::new();
    let param = PARAM
        .get_or_init(|| Regex::new(&format!("[?&]{key}=(\\d+)")).expect("静态正则必然编译成功"));
    param
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string())
}

/// 从 HTML 片段中提取第一个 img 的 src。
fn extract_img_src(html: &str) -> Option<String> {
    static IMG: OnceLock<Regex> = OnceLock::new();
    let img =
        IMG.get_or_init(|| Regex::new(r#"<img[^>]+src="([^"]+)"#).expect("静态正则必然编译成功"));
    img.captures(html)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string())
}

/// 站内相对路径补全为绝对地址。
fn absolutize_url(src: &str) -> String {
    if src.starts_with("http") {
        src.to_string()
    } else {
        let trimmed = src.trim_start_matches("./");
        format!("{BASE_URL}/{trimmed}")
    }
}

/// 解析团队详情子页，把子页独有的字段补进列表页条目。
/// 调用方需保证 html 含 "Team Profile"（子页校验在外层完成）。
pub(crate) fn parse_team_sub(html: &str, mut entry: TeamEntry) -> TeamEntry {
    static SCORE_FS: OnceLock<Regex> = OnceLock::new();
    static COUNTRY: OnceLock<Regex> = OnceLock::new();
    let score_fs = SCORE_FS
        .get_or_init(|| Regex::new(r#"id="score_fs">([^<]+)<"#).expect("静态正则必然编译成功"));
    let country = COUNTRY.get_or_init(|| {
        Regex::new(r#"Country <img[^>]*title="([A-Za-z]+)"#).expect("静态正则必然编译成功")
    });
    let counter = Selector::parse(r#"div[id="ratiopoint"]"#).expect("静态选择器必然解析成功");
    entry.ratio_points = {
        let document = Html::parse_document(html);
        document
            .select(&counter)
            .map(|element| clean_cell_text(element))
            .collect()
    };
    entry.final_striker = score_fs
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().trim().to_string());
    entry.leader_country = country
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string());

    entry.team_genre = section_after(html, "チームジャンル");
    entry.team_common = section_after(html, "チームの共通点");
    entry.team_reason = section_after(html, "チームを結成した理由");

    // Member List 表：th 含 名前 的三列表
    let document = Html::parse_document(html);
    for table in document.select(&Selector::parse("table").expect("静态选择器必然解析成功"))
    {
        let head: String = table
            .select(&Selector::parse("th").expect("静态选择器必然解析成功"))
            .map(|th| clean_cell_text(th))
            .collect::<Vec<_>>()
            .join("|");
        if !head.contains("名前") {
            continue;
        }
        for row in table.select(&Selector::parse("tr").expect("静态选择器必然解析成功"))
        {
            let cells: Vec<String> = row
                .select(&Selector::parse("td").expect("静态选择器必然解析成功"))
                .map(|td| clean_cell_text(td))
                .collect();
            if cells.len() < 2 || cells[0].is_empty() || cells[0] == "合計" {
                continue;
            }
            entry.member_rows.push(TeamMember {
                name: cells[0].clone(),
                role: cells[1].clone(),
                site: cells.get(2).filter(|s| !s.is_empty()).cloned(),
            });
        }
    }

    entry
}

/// 取 `<h3>` 标题之后的区块文本（到下一个 `<h3>` 为止），用于团队子页的自由文本区。
fn section_after(html: &str, heading: &str) -> Option<String> {
    let pos = html.find(heading)?;
    let rest = &html[pos..];
    let h3_end = rest.find("</h3>")?;
    let after = &rest[h3_end + "</h3>".len()..];
    let segment = match after.find("<h3") {
        Some(next) => &after[..next],
        None => after,
    };
    let text = html_to_text(segment);
    (!text.is_empty()).then_some(text)
}

/// 解析单作品详情页（`event.cgi?action=More_def`）。
///
/// 页面为字段行（字段名/值，部分行带附加列）加自由文本区（試聴、TAG
/// 展开、コメント）。至少必须解析出 Title 字段行，否则视为回退页报错。
/// 从 `More_def` 页面的表格字段行提取字段；返回是否存在 `Title` 行。
fn extract_more_def_fields(document: &Html, detail: &mut EntryDetail) -> bool {
    static ROW: OnceLock<Selector> = OnceLock::new();
    static CELL: OnceLock<Selector> = OnceLock::new();
    let row_selector = ROW.get_or_init(|| Selector::parse("tr").expect("静态选择器必然解析成功"));
    let cell_selector =
        CELL.get_or_init(|| Selector::parse("td, th").expect("静态选择器必然解析成功"));
    let mut has_title = false;

    for row in document.select(row_selector) {
        let cells_html: Vec<String> = row
            .select(cell_selector)
            .map(|cell| cell.inner_html())
            .collect();
        let cells: Vec<String> = row
            .select(cell_selector)
            .map(|cell| clean_cell_text(cell))
            .collect();
        if cells.is_empty() {
            continue;
        }
        let field = normalize_header(&cells[0]);
        let value =
            |i: usize| -> Option<String> { cells.get(i).filter(|text| !text.is_empty()).cloned() };
        match field.as_str() {
            "title" => has_title = true,
            "bms artist" => detail.artist = value(1),
            "genre" => {
                detail.genre = value(1);
                detail.genre_sub = value(2);
            }
            "source" => detail.source = value(1),
            "size" => detail.bga = value(3),
            "charts" => {
                detail.charts = value(1);
                detail.bpm = value(3);
            }
            "tag" => {
                if let Some(cell_html) = cells_html.get(1) {
                    detail.tags = html_to_text(cell_html)
                        .split_whitespace()
                        .map(str::to_string)
                        .collect();
                }
            }
            "製作環境" => detail.production = value(1),
            _ => {}
        }
    }

    has_title
}

pub(crate) fn parse_more_def(html: &str, num: &str) -> Result<EntryDetail> {
    static LATEST: OnceLock<Regex> = OnceLock::new();
    static UPDATED: OnceLock<Regex> = OnceLock::new();
    let latest = LATEST.get_or_init(|| {
        Regex::new(r"<strong>Revision : (\d+) / [^<]*</strong><br ?/?>\s*([^<\s][^<]*)")
            .expect("静态正则必然编译成功")
    });
    let updated = UPDATED.get_or_init(|| {
        Regex::new(r"\((\d{4}年\d{1,2}月\d{1,2}日 \d{1,2}:\d{2}) 更新\)")
            .expect("静态正则必然编译成功")
    });

    let mut detail = EntryDetail {
        num: num.to_string(),
        artist: None,
        genre: None,
        genre_sub: None,
        source: None,
        bga: None,
        charts: None,
        bpm: None,
        tags: Vec::new(),
        production: None,
        audition: Vec::new(),
        comment_updated: None,
        comment: None,
        revisions: Vec::new(),
    };

    let document = Html::parse_document(html);
    let has_title = extract_more_def_fields(&document, &mut detail);
    if !has_title {
        bail!("More_def: 未找到 Title 字段行（可能为回退页）");
    }

    // 作品更新履历：嵌套的 memberlist_output 表 + 最新一条摘要（仅在 strong 里）
    let revision_table =
        Selector::parse("table.memberlist_output").expect("静态选择器必然解析成功");
    let row_in_table = Selector::parse("tr").expect("静态选择器必然解析成功");
    let td = Selector::parse("td").expect("静态选择器必然解析成功");
    for row in document
        .select(&revision_table)
        .flat_map(|table| table.select(&row_in_table))
    {
        let cells: Vec<String> = row.select(&td).map(|cell| clean_cell_text(cell)).collect();
        if cells.len() == 3 && !cells[0].is_empty() {
            detail.revisions.push(Revision {
                ver: cells[0].clone(),
                date: cells[1].clone(),
                note: cells[2].clone(),
            });
        }
    }
    if let Some(captures) = latest.captures(html) {
        detail.revisions.push(Revision {
            ver: captures[1].to_string(),
            date: String::new(),
            note: captures[2].trim().to_string(),
        });
    }

    let audition_sel = Selector::parse("div.m_audition").expect("静态选择器必然解析成功");
    for element in document.select(&audition_sel) {
        let url = clean_cell_text(element);
        if !url.is_empty() {
            detail.audition.push(url);
        }
    }

    let comment_sel = Selector::parse("div.moretext_comment").expect("静态选择器必然解析成功");
    if let Some(element) = document.select(&comment_sel).next() {
        let inner = element.inner_html();
        detail.comment_updated = updated
            .captures(&inner)
            .and_then(|captures| captures.get(1))
            .map(|m| m.as_str().to_string());
        detail.comment = Some(html_to_text(&inner));
    }

    Ok(detail)
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

/// 把 HTML 片段转成纯文本：br 与块级标签转为换行，剥掉其余标签并解码实体。
fn html_to_text(html: &str) -> String {
    static BR: OnceLock<Regex> = OnceLock::new();
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    let br = BR.get_or_init(|| Regex::new(r"(?i)<br\s*/?>").expect("静态正则必然编译成功"));
    let block = BLOCK.get_or_init(|| {
        Regex::new(r"(?i)</?(div|p|h[1-6]|li|tr|table|ul|ol)[^>]*>").expect("静态正则必然编译成功")
    });

    let after_br = br.replace_all(html, "\n");
    let with_breaks = block.replace_all(&after_br, "\n");
    let fragment = scraper::Html::parse_fragment(&with_breaks);
    let raw: String = fragment.root_element().text().collect::<Vec<_>>().join("");

    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
