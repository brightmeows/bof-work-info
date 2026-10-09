//! 数据模型：事件数据文件与抓取结果的共用类型。

use serde::{Deserialize, Serialize};

/// 单个作品条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BmsEntry {
    pub(crate) no: String,
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) team: Option<String>,
    pub(crate) title: String,
    pub(crate) size: String,
    pub(crate) addr: Vec<String>,
    // 以下为 sp 页（报名一览）补充的评分统计字段；无 sp 数据时缺省。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) impr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) total: Option<String>,
    /// sp 表头的 Total(N) 列；部分年代的事件没有该列。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) total_n: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) median: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) avg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) regist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) update: Option<String>,
}

/// 团队档案条目（`event_teamprofile.cgi`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TeamEntry {
    pub(crate) no: String,
    pub(crate) team: String,
    pub(crate) leader: String,
    /// 成员数，保留页面原文（如 "4人"）。
    pub(crate) member: String,
    /// 作品数，保留页面原文（如 "0 / 4作品"）。
    pub(crate) works: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) regist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) update: Option<String>,
    // 以下为列表页与团队详情子页的补充字段；无对应页面时缺省。
    /// 子页链接中的稳定团队 id（如 "175"）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) team_id: Option<String>,
    /// 徽章/横幅图片链接。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) banner: Option<String>,
    /// 成员名单文本（含担任分工）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) members: Option<String>,
    /// 队长国籍（子页）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) leader_country: Option<String>,
    /// Ratio Point 1/2/3（子页）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) ratio_points: Vec<String>,
    /// FINAL STRIKER 倍率（子页）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) final_striker: Option<String>,
    /// 团队 genre（子页）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) team_genre: Option<String>,
    /// 团队共通点（子页）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) team_common: Option<String>,
    /// 团队结成理由（子页）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) team_reason: Option<String>,
    /// 成员明细（子页，名前/担当/サイト）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) member_rows: Vec<TeamMember>,
}

impl TeamEntry {
    /// stdin 调试模式的最小条目骨架；字段由子页解析补全。
    pub(crate) fn stub() -> Self {
        Self {
            no: String::new(),
            team: String::new(),
            leader: String::new(),
            member: String::new(),
            works: String::new(),
            regist: None,
            update: None,
            team_id: None,
            banner: None,
            members: None,
            leader_country: None,
            ratio_points: Vec::new(),
            final_striker: None,
            team_genre: None,
            team_common: None,
            team_reason: None,
            member_rows: Vec::new(),
        }
    }
}

/// 团队成员明细行（团队详情子页）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TeamMember {
    pub(crate) name: String,
    pub(crate) role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) site: Option<String>,
}

/// 单个事件的数据文件内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BmsData {
    pub(crate) entries: Vec<BmsEntry>,
    /// 团队档案；仅进行中的活动提供该页，空时不输出该段。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) teams: Vec<TeamEntry>,
}

/// 站点上发现的事件。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ManbowEvent {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
}

/// 单作品详情（`More_def` 页）；仅进行中活动抓取。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EntryDetail {
    /// 对应 entries 的 no。
    pub(crate) num: String,
    /// BMS Artist 行（含 movie/illustration 等协作署名）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) genre_sub: Option<String>,
    /// 原曲来源标注。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<String>,
    /// BGA 标注（如 "BGA include・Other"）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bga: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) charts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bpm: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tags: Vec<String>,
    /// 製作環境。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) production: Option<String>,
    /// 試聴链接。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) audition: Vec<String>,
    /// 作者/团队评论的更新时间戳。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) comment_updated: Option<String>,
    /// 作者/团队评论全文（纯文本）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) comment: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) revisions: Vec<Revision>,
}

/// 作品更新履历行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Revision {
    pub(crate) ver: String,
    pub(crate) date: String,
    pub(crate) note: String,
}

/// 单事件的作品详情文件内容（`events/<id>.details.toml`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DetailsData {
    pub(crate) details: Vec<EntryDetail>,
}
