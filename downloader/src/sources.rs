use std::{
    any::Any,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BmsEntry {
    pub no: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    pub title: String,
    pub size: String,
    pub addr: Vec<String>,
}

pub trait LinkTypeTrait: std::fmt::Debug + Any {
    fn is_downloadable(&self) -> bool;
    fn get_direct_url(&self) -> Option<String>;
    #[allow(unused)]
    fn get_url(&self) -> &str;
    #[allow(unused)]
    fn get_type_name(&self) -> &'static str;
    fn from_url(url: &str) -> Option<Self>
    where
        Self: Sized;

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone)]
pub struct DirectLink {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct GoogleDriveLink {
    pub share_id: String,
}

#[derive(Debug, Clone)]
pub struct DropboxLink {
    pub share_id: String,
}

#[derive(Debug, Clone)]
pub struct OneDriveLink {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct MediaFireLink {
    pub url: String,
}

// 为所有具体类型实现Debug trait以便格式化输出
pub type LinkType = Box<dyn LinkTypeTrait>;

// 为每个具体类型实现 LinkTypeTrait
impl LinkTypeTrait for DirectLink {
    fn is_downloadable(&self) -> bool {
        true
    }
    fn get_direct_url(&self) -> Option<String> {
        Some(self.url.clone())
    }
    fn get_url(&self) -> &str {
        &self.url
    }
    fn get_type_name(&self) -> &'static str {
        "Direct"
    }
    fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("http://") || url.starts_with("https://") {
            Some(DirectLink {
                url: url.to_string(),
            })
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LinkTypeTrait for GoogleDriveLink {
    fn is_downloadable(&self) -> bool {
        true
    }
    fn get_direct_url(&self) -> Option<String> {
        if self.share_id.starts_with("https://") {
            Some(self.share_id.clone())
        } else {
            Some(format!(
                "https://drive.google.com/uc?export=download&id={}",
                self.share_id
            ))
        }
    }
    fn get_url(&self) -> &str {
        &self.share_id
    }
    fn get_type_name(&self) -> &'static str {
        "GoogleDrive"
    }
    fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("https://drive.google.com/")
            || url.starts_with("https://drive.usercontent.google.com/")
        {
            if let Some(id) = extract_google_drive_id(url) {
                Some(GoogleDriveLink { share_id: id })
            } else {
                Some(GoogleDriveLink {
                    share_id: url.to_string(),
                })
            }
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LinkTypeTrait for DropboxLink {
    fn is_downloadable(&self) -> bool {
        true
    }
    fn get_direct_url(&self) -> Option<String> {
        if self.share_id.contains("?dl=0") {
            Some(self.share_id.replace("?dl=0", "?dl=1"))
        } else if !self.share_id.contains("?dl=") {
            Some(format!("{}?dl=1", self.share_id))
        } else {
            Some(self.share_id.clone())
        }
    }
    fn get_url(&self) -> &str {
        &self.share_id
    }
    fn get_type_name(&self) -> &'static str {
        "Dropbox"
    }
    fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("https://www.dropbox.com/")
            || url.starts_with("https://dl.dropboxusercontent.com/")
        {
            if let Some(id) = extract_dropbox_id(url) {
                Some(DropboxLink { share_id: id })
            } else {
                Some(DropboxLink {
                    share_id: url.to_string(),
                })
            }
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LinkTypeTrait for OneDriveLink {
    fn is_downloadable(&self) -> bool {
        true
    }
    fn get_direct_url(&self) -> Option<String> {
        Some(self.url.clone())
    }
    fn get_url(&self) -> &str {
        &self.url
    }
    fn get_type_name(&self) -> &'static str {
        "OneDrive"
    }
    fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("https://1drv.ms/") {
            Some(OneDriveLink {
                url: url.to_string(),
            })
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LinkTypeTrait for MediaFireLink {
    fn is_downloadable(&self) -> bool {
        true
    }
    fn get_direct_url(&self) -> Option<String> {
        Some(self.url.clone())
    }
    fn get_url(&self) -> &str {
        &self.url
    }
    fn get_type_name(&self) -> &'static str {
        "MediaFire"
    }
    fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("https://www.mediafire.com/") {
            Some(MediaFireLink {
                url: url.to_string(),
            })
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn create_link_from_url(url: &str) -> LinkType {
    // 尝试各种链接类型
    if let Some(link) = DirectLink::from_url(url) {
        Box::new(link)
    } else if let Some(link) = GoogleDriveLink::from_url(url) {
        Box::new(link)
    } else if let Some(link) = DropboxLink::from_url(url) {
        Box::new(link)
    } else if let Some(link) = OneDriveLink::from_url(url) {
        Box::new(link)
    } else if let Some(link) = MediaFireLink::from_url(url) {
        Box::new(link)
    } else {
        // 默认为DirectLink
        Box::new(DirectLink {
            url: url.to_string(),
        })
    }
}

pub fn extract_google_drive_id(url: &str) -> Option<String> {
    // 匹配格式: https://drive.google.com/file/d/ID/view
    if let Some(start) = url.find("/file/d/") {
        let id_start = start + 8;
        if let Some(end) = url[id_start..].find('/') {
            return Some(url[id_start..id_start + end].to_string());
        }
    }

    // 匹配格式: https://drive.google.com/uc?id=ID
    if let Some(start) = url.find("?id=") {
        let id_start = start + 4;
        if let Some(end) = url[id_start..].find('&') {
            return Some(url[id_start..id_start + end].to_string());
        }
        return Some(url[id_start..].to_string());
    }

    // 匹配格式: https://drive.usercontent.google.com/download?id=ID
    if let Some(start) = url.find("/download?id=") {
        let id_start = start + 13;
        if let Some(end) = url[id_start..].find('&') {
            return Some(url[id_start..id_start + end].to_string());
        }
        return Some(url[id_start..].to_string());
    }

    // 匹配格式: https://drive.usercontent.google.com/u/0/uc?id=ID
    if let Some(start) = url.find("/uc?id=") {
        let id_start = start + 7;
        if let Some(end) = url[id_start..].find('&') {
            return Some(url[id_start..id_start + end].to_string());
        }
        return Some(url[id_start..].to_string());
    }

    None
}

pub fn extract_dropbox_id(url: &str) -> Option<String> {
    // 匹配格式: https://www.dropbox.com/s/ID/filename
    if let Some(start) = url.find("/s/") {
        let id_start = start + 3;
        if let Some(end) = url[id_start..].find('/') {
            return Some(url[id_start..id_start + end].to_string());
        }
    }

    // 匹配格式: https://www.dropbox.com/scl/fi/ID/filename
    // 匹配格式: https://www.dropbox.com/scl/fo/ID/filename
    // 匹配格式: https://dl.dropboxusercontent.com/scl/fi/ID/filename
    for pattern in ["/scl/fi/", "/scl/fo/"] {
        if let Some(start) = url.find(pattern) {
            let id_start = start + pattern.len();
            if let Some(end) = url[id_start..].find('/') {
                return Some(url[id_start..id_start + end].to_string());
            }
        }
    }

    // 特殊处理 dropboxusercontent.com 格式
    if url.starts_with("https://dl.dropboxusercontent.com/scl/fi/") {
        let id_start = 40; // "https://dl.dropboxusercontent.com/scl/fi/".len()
        if let Some(end) = url[id_start..].find('/') {
            return Some(url[id_start..id_start + end].to_string());
        }
    }

    None
}

pub fn is_valid_url(url: &str) -> bool {
    // 基本URL格式验证
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    // 检查是否包含有效的域名部分
    if let Some(colon_pos) = url.find("://") {
        let after_protocol = &url[colon_pos + 3..];
        if after_protocol.is_empty() || !after_protocol.contains('.') {
            return false;
        }
    } else {
        return false;
    }

    // 可以添加更多URL格式验证规则
    true
}

pub fn analyze_links(entry: &BmsEntry) -> (Vec<LinkType>, Vec<String>) {
    let mut links = Vec::new();
    let mut non_links = Vec::new();

    for addr in &entry.addr {
        // 只接受以 http:// 或 https:// 开头的有效URL
        if addr.starts_with("http://") || addr.starts_with("https://") {
            // 进一步验证URL格式
            if is_valid_url(addr) {
                links.push(create_link_from_url(addr));
            } else {
                non_links.push(addr.clone());
            }
        } else {
            non_links.push(addr.clone());
        }
    }

    (links, non_links)
}

pub fn select_download_link_for_trait(
    entry: &BmsEntry,
    interactive: bool,
) -> Result<Option<Box<dyn LinkTypeTrait>>> {
    let (links, non_links) = analyze_links(entry);

    if links.is_empty() {
        warn!("作品 #{} - {} 没有可下载的链接", entry.no, entry.title);
        if !non_links.is_empty() {
            info!("  非链接内容: {}", non_links.join(", "));
        }
        return Ok(None);
    }

    let downloadable_links: Vec<Box<dyn LinkTypeTrait>> = links
        .into_iter()
        .filter(|link| link.is_downloadable())
        .collect();

    if downloadable_links.is_empty() {
        warn!("作品 #{} - {} 没有支持的下载链接", entry.no, entry.title);
        // 重新获取所有链接来显示调试信息
        let all_links = analyze_links(entry).0;
        info!(
            "  可用链接: {}",
            all_links
                .iter()
                .map(|l| format!("{l:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Ok(None);
    }

    if downloadable_links.len() == 1 {
        // 只有一个可下载链接，直接使用
        let link = downloadable_links.into_iter().next().unwrap();
        info!(
            "作品 #{} - {} 使用唯一链接: {}",
            entry.no,
            entry.title,
            link.get_direct_url().unwrap_or_default()
        );
        return Ok(Some(link));
    }

    // 多个链接的情况
    if interactive {
        println!("\n作品 #{} - {}", entry.no, entry.title);
        println!("作者: {}", entry.name);
        if let Some(team) = &entry.team {
            println!("团队: {team}");
        }
        println!("大小: {}", entry.size);
        println!("\n可用的下载链接:");

        // 重新获取所有链接来显示调试信息
        let all_links = analyze_links(entry).0;
        let downloadable_links_for_display: Vec<_> = all_links
            .iter()
            .filter(|link| link.is_downloadable())
            .collect();
        for (i, link) in downloadable_links_for_display.iter().enumerate() {
            let direct_url = link
                .get_direct_url()
                .unwrap_or_else(|| "无法获取直接链接".to_string());
            println!("  {}. {:?} -> {}", i + 1, link, direct_url);
        }

        println!("\n请选择要下载的链接 (输入数字，或按 Enter 跳过):");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if let Ok(choice) = input.trim().parse::<usize>()
            && choice > 0
            && choice <= downloadable_links.len()
        {
            let selected_link = downloadable_links.into_iter().nth(choice - 1).unwrap();
            return Ok(Some(selected_link));
        }

        info!("跳过作品 #{} - {}", entry.no, entry.title);
        Ok(None)
    } else {
        // 非交互模式，提示用户
        warn!(
            "作品 #{} - {} 有多个下载链接，请使用 --interactive 模式选择:",
            entry.no, entry.title
        );
        // 重新获取所有链接来显示调试信息
        let all_links = analyze_links(entry).0;
        let downloadable_links_for_display: Vec<_> = all_links
            .iter()
            .filter(|link| link.is_downloadable())
            .collect();
        for (i, link) in downloadable_links_for_display.iter().enumerate() {
            let direct_url = link
                .get_direct_url()
                .unwrap_or_else(|| "无法获取直接链接".to_string());
            println!("  {}. {:?} -> {}", i + 1, link, direct_url);
        }
        Ok(None)
    }
}

pub async fn download_google_drive_file(file_id: &str, output_path: &Path) -> Result<PathBuf> {
    info!(
        "下载Google Drive文件: {file_id} -> {}",
        output_path.display()
    );

    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 创建HTTP客户端
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| anyhow::anyhow!("创建HTTP客户端失败: {e}"))?;

    // 第一步：获取cookie和确认页面
    let cookie_path = output_path.parent().unwrap().join("cookie");
    let cookie_url = format!("https://drive.google.com/uc?export=download&id={file_id}");

    info!("第一步：获取cookie，URL: {cookie_url}");

    let cookie_response = client
        .get(&cookie_url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("获取cookie失败: {cookie_url} - {e}"))?;

    // 保存cookie内容
    let cookie_content = cookie_response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("读取cookie响应失败: {cookie_url} - {e}"))?;

    info!("Cookie内容长度: {} 字节", cookie_content.len());

    fs::write(&cookie_path, &cookie_content)
        .with_context(|| format!("保存cookie失败: {}", cookie_path.display()))?;

    // 从HTML内容中提取下载URL和文件名
    let download_url = if let Some(url) = extract_download_url_from_html(&cookie_content) {
        info!("从HTML中提取到下载URL: {url}");
        url
    } else {
        // 如果没有找到完整的下载URL，尝试使用确认token
        let confirm_token = extract_confirm_token_from_html(&cookie_content);
        info!("提取到的确认token: '{confirm_token}'");

        if confirm_token.is_empty() {
            info!("没有找到确认token，直接使用原始URL");
            cookie_url.clone()
        } else {
            let url = format!(
                "https://drive.google.com/uc?export=download&confirm={confirm_token}&id={file_id}"
            );
            info!("使用确认token构造下载URL: {url}");
            url
        }
    };

    // 尝试从HTML中提取原始文件名
    let original_filename = extract_filename_from_html(&cookie_content);

    info!("第二步：下载文件，URL: {download_url}");

    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("下载文件失败: {download_url} - {e}"))?;

    // 检查响应状态
    let status = response.status();
    info!("HTTP响应状态: {status}");

    // 尝试从Content-Disposition头中提取文件名
    let mut filename = None;
    if let Some(disposition) = response.headers().get("Content-Disposition") {
        let disposition_str = disposition.to_str().unwrap_or("");
        info!("Content-Disposition: {disposition_str}");
        filename = extract_filename_from_disposition(disposition_str);
    }

    // 确定最终的文件名：优先使用Content-Disposition，然后是HTML中的文件名，最后是默认名称
    let final_output_path = if let Some(header_filename) = &filename {
        let parent_dir = output_path.parent().unwrap();
        let new_path = parent_dir.join(header_filename);
        info!(
            "使用Content-Disposition中的文件名: {header_filename} -> {}",
            new_path.display()
        );
        new_path
    } else if let Some(html_filename) = &original_filename {
        let parent_dir = output_path.parent().unwrap();
        let new_path = parent_dir.join(html_filename);
        info!(
            "使用HTML中的文件名: {html_filename} -> {}",
            new_path.display()
        );
        new_path
    } else {
        output_path.to_path_buf()
    };

    let mut file = fs::File::create(&final_output_path)
        .with_context(|| format!("创建文件失败: {}", final_output_path.display()))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!("读取响应失败: {download_url} - {e}"))?;

    info!("下载的数据长度: {} 字节", bytes.len());

    std::io::Write::write_all(&mut file, &bytes)
        .with_context(|| format!("写入文件失败: {}", final_output_path.display()))?;

    // 清理cookie文件
    let _ = fs::remove_file(&cookie_path);

    info!(
        "Google Drive下载完成: {} ({} 字节)",
        final_output_path.display(),
        bytes.len()
    );
    Ok(final_output_path)
}

pub fn extract_filename_from_disposition(disposition: &str) -> Option<String> {
    // 解析 Content-Disposition 头，格式如：
    // attachment; filename="filename.zip"
    // attachment; filename*=UTF-8''filename.zip
    if let Some(start) = disposition.find("filename=") {
        let filename_start = start + 9;
        let filename_part = &disposition[filename_start..];

        // 处理引号包围的文件名
        if let Some(stripped) = filename_part.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                return Some(stripped[..end].to_string());
            }
        }
        // 处理没有引号的文件名
        else if let Some(end) = filename_part.find(';') {
            return Some(filename_part[..end].to_string());
        } else {
            return Some(filename_part.to_string());
        }
    }

    // 处理 filename*=UTF-8'' 格式
    if let Some(start) = disposition.find("filename*=UTF-8''") {
        let filename_start = start + 16;
        let filename_part = &disposition[filename_start..];
        if let Some(end) = filename_part.find(';') {
            return Some(filename_part[..end].to_string());
        }
        return Some(filename_part.to_string());
    }

    None
}

pub fn extract_confirm_token_from_html(html_content: &str) -> String {
    // 在HTML中查找确认token
    // 通常格式为: <a href="/uc?export=download&confirm=TOKEN&id=FILE_ID"
    if let Some(start) = html_content.find("confirm=") {
        let token_start = start + 8;
        if let Some(end) = html_content[token_start..].find('&') {
            return html_content[token_start..token_start + end].to_string();
        } else if let Some(end) = html_content[token_start..].find('"') {
            return html_content[token_start..token_start + end].to_string();
        }
        return html_content[token_start..].to_string();
    }

    // 也尝试查找其他可能的格式
    if let Some(start) = html_content.find("&confirm=") {
        let token_start = start + 9;
        if let Some(end) = html_content[token_start..].find('&') {
            return html_content[token_start..token_start + end].to_string();
        } else if let Some(end) = html_content[token_start..].find('"') {
            return html_content[token_start..token_start + end].to_string();
        }
        return html_content[token_start..].to_string();
    }

    String::new()
}

pub fn extract_download_url_from_html(html_content: &str) -> Option<String> {
    // 查找隐藏字段的值
    let mut id = None;
    let mut export = None;
    let mut confirm = None;
    let mut uuid = None;

    // 提取id
    if let Some(start) = html_content.find("name=\"id\" value=\"") {
        let value_start = start + 17;
        if let Some(end) = html_content[value_start..].find('"') {
            id = Some(html_content[value_start..value_start + end].to_string());
        }
    }

    // 提取export
    if let Some(start) = html_content.find("name=\"export\" value=\"") {
        let value_start = start + 20;
        if let Some(end) = html_content[value_start..].find('"') {
            export = Some(html_content[value_start..value_start + end].to_string());
        }
    }

    // 提取confirm
    if let Some(start) = html_content.find("name=\"confirm\" value=\"") {
        let value_start = start + 21;
        if let Some(end) = html_content[value_start..].find('"') {
            confirm = Some(html_content[value_start..value_start + end].to_string());
        }
    }

    // 提取uuid
    if let Some(start) = html_content.find("name=\"uuid\" value=\"") {
        let value_start = start + 19;
        if let Some(end) = html_content[value_start..].find('"') {
            uuid = Some(html_content[value_start..value_start + end].to_string());
        }
    }

    info!("提取到的参数: id={id:?}, export={export:?}, confirm={confirm:?}, uuid={uuid:?}");

    // 构造完整的下载URL
    if let (Some(id_val), Some(export_val), Some(confirm_val), Some(uuid_val)) =
        (id, export, confirm, uuid)
    {
        let url = format!(
            "https://drive.usercontent.google.com/download?id={id_val}&export={export_val}&confirm={confirm_val}&uuid={uuid_val}"
        );
        info!("构造的下载URL: {url}");
        return Some(url);
    }

    None
}

pub fn extract_filename_from_html(html_content: &str) -> Option<String> {
    // 从HTML中提取文件名，格式如: <a href="/open?id=...">filename.zip</a>
    if let Some(start) = html_content.find('>')
        && let Some(end) = html_content[start + 1..].find('<')
    {
        let filename = &html_content[start + 1..start + 1 + end];
        if filename.contains('.') && !filename.contains(' ') {
            info!("从HTML中提取到文件名: {filename}");
            return Some(filename.to_string());
        }
    }

    None
}

pub fn is_valid_archive(file_path: &Path) -> Result<bool> {
    let file = fs::File::open(file_path)
        .with_context(|| format!("无法打开文件: {}", file_path.display()))?;

    let mut reader = BufReader::new(file);
    let mut header = [0u8; 4];

    if reader.read_exact(&mut header).is_err() {
        return Ok(false);
    }

    // 检查ZIP文件头 (PK)
    if header[0] == 0x50 && header[1] == 0x4B {
        return Ok(true);
    }

    // 检查RAR文件头
    if header[0] == 0x52 && header[1] == 0x61 && header[2] == 0x72 && header[3] == 0x21 {
        return Ok(true);
    }

    // 检查7Z文件头
    if header[0] == 0x37 && header[1] == 0x7A && header[2] == 0xBC && header[3] == 0xAF {
        return Ok(true);
    }

    // 检查TAR文件头 (需要更多字节，但先检查前4个)
    if header[0] == 0x75 && header[1] == 0x73 && header[2] == 0x74 && header[3] == 0x61 {
        return Ok(true);
    }

    Ok(false)
}

pub async fn download_file(url: &str, output_path: &Path) -> Result<PathBuf> {
    info!("下载: {url} -> {}", output_path.display());

    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let response = reqwest::get(url).await?;

    let mut file = fs::File::create(output_path)
        .with_context(|| format!("创建文件失败: {}", output_path.display()))?;

    let bytes = response.bytes().await?;

    std::io::Write::write_all(&mut file, &bytes)
        .with_context(|| format!("写入文件失败: {}", output_path.display()))?;

    info!("下载完成: {}", output_path.display());
    Ok(output_path.to_path_buf())
}

pub fn generate_filename(entry: &BmsEntry) -> String {
    let mut filename = format!("{} - {}", entry.no, entry.title);

    // 清理文件名中的非法字符
    filename = filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

    // 如果文件名太长，截断
    if filename.len() > 100 {
        filename = format!("{}...", &filename[..97]);
    }

    filename
}

pub async fn download_entry(entry: &BmsEntry, output_dir: &Path, interactive: bool) -> Result<()> {
    let filename = generate_filename(entry);
    let output_path = output_dir.join(&filename);

    if let Some(link) = select_download_link_for_trait(entry, interactive)? {
        let final_path = download_link_by_type(link.as_ref(), &output_path).await?;

        // 验证下载的文件是否为有效压缩包
        if is_valid_archive(&final_path)? {
            info!("文件验证成功: {} 是有效的压缩包", final_path.display());
        } else {
            warn!("文件验证失败: {} 不是有效的压缩包", final_path.display());
        }
    }

    Ok(())
}

pub fn extract_google_drive_id_from_url(url: &str) -> Option<String> {
    // 匹配格式: https://drive.google.com/file/d/ID/view
    if let Some(start) = url.find("/file/d/") {
        let id_start = start + 8;
        if let Some(end) = url[id_start..].find('/') {
            return Some(url[id_start..id_start + end].to_string());
        }
    }

    // 匹配格式: https://drive.google.com/uc?id=ID 或 https://drive.google.com/uc?export=download&id=ID
    if let Some(start) = url.find("?id=") {
        let id_start = start + 4;
        if let Some(end) = url[id_start..].find('&') {
            return Some(url[id_start..id_start + end].to_string());
        }
        return Some(url[id_start..].to_string());
    }

    // 匹配格式: https://drive.google.com/uc?export=download&id=ID
    if let Some(start) = url.find("&id=") {
        let id_start = start + 4;
        if let Some(end) = url[id_start..].find('&') {
            return Some(url[id_start..id_start + end].to_string());
        }
        return Some(url[id_start..].to_string());
    }

    None
}

pub async fn download_link_by_type(
    link: &dyn LinkTypeTrait,
    output_path: &Path,
) -> Result<PathBuf> {
    let type_name = link.get_type_name();

    match type_name {
        "Direct" => {
            let direct_link = link.as_any().downcast_ref::<DirectLink>().unwrap();
            download_file(&direct_link.url, output_path).await
        }
        "GoogleDrive" => {
            let google_link = link.as_any().downcast_ref::<GoogleDriveLink>().unwrap();
            let file_id = if google_link.share_id.starts_with("https://") {
                if let Some(id) = extract_google_drive_id_from_url(&google_link.share_id) {
                    id
                } else {
                    return Err(anyhow::anyhow!(
                        "无法从Google Drive URL中提取文件ID: {}",
                        google_link.share_id
                    ));
                }
            } else {
                google_link.share_id.clone()
            };
            download_google_drive_file(&file_id, output_path).await
        }
        "Dropbox" => {
            let dropbox_link = link.as_any().downcast_ref::<DropboxLink>().unwrap();
            let download_url = dropbox_link.get_direct_url().ok_or_else(|| {
                anyhow::anyhow!("无法获取Dropbox下载链接: {}", dropbox_link.share_id)
            })?;
            download_file(&download_url, output_path).await
        }
        "OneDrive" => {
            let onedrive_link = link.as_any().downcast_ref::<OneDriveLink>().unwrap();
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .map_err(|e| anyhow::anyhow!("创建HTTP客户端失败: {e}"))?;

            let response = client.head(&onedrive_link.url).send().await.map_err(|e| {
                anyhow::anyhow!("访问OneDrive链接失败: {} - {}", onedrive_link.url, e)
            })?;

            let final_url = response.url().to_string();
            download_file(&final_url, output_path).await
        }
        "MediaFire" => {
            let mediafire_link = link.as_any().downcast_ref::<MediaFireLink>().unwrap();
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .map_err(|e| anyhow::anyhow!("创建HTTP客户端失败: {e}"))?;

            let response = client.get(&mediafire_link.url).send().await.map_err(|e| {
                anyhow::anyhow!("访问MediaFire链接失败: {} - {}", mediafire_link.url, e)
            })?;

            let html_content = response.text().await.map_err(|e| {
                anyhow::anyhow!("读取MediaFire页面失败: {} - {}", mediafire_link.url, e)
            })?;

            let download_url = extract_mediafire_download_url(&html_content).ok_or_else(|| {
                anyhow::anyhow!("无法从MediaFire页面中提取下载链接: {}", mediafire_link.url)
            })?;

            download_file(&download_url, output_path).await
        }
        _ => Err(anyhow::anyhow!("不支持的链接类型: {type_name}")),
    }
}

pub fn extract_mediafire_download_url(html_content: &str) -> Option<String> {
    // 在MediaFire页面中查找下载按钮的链接
    // 通常格式为: <a href="下载链接" class="download_link" 或包含 download
    if let Some(start) = html_content.find("download_link")
        && let Some(href_start) = html_content[..start].rfind("href=\"")
    {
        let url_start = href_start + 6;
        if let Some(end) = html_content[url_start..].find('"') {
            let url = &html_content[url_start..url_start + end];
            if url.starts_with("http") {
                return Some(url.to_string());
            }
        }
    }

    // 尝试查找其他可能的下载链接模式
    if let Some(start) = html_content.find("http") {
        let potential_url = &html_content[start..];
        if let Some(end) = potential_url.find('"') {
            let url = &potential_url[..end];
            if url.contains("download") && (url.contains("mediafire") || url.contains("dl")) {
                return Some(url.to_string());
            }
        }
    }

    None
}
