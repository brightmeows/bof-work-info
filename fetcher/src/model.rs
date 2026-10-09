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
