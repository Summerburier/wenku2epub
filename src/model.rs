use std::collections::HashMap;

/// 一本小说的完整数据模型，贯穿下载全程
#[derive(Debug, Default)]
pub struct Book {
    /// 书名
    pub title: String,
    /// 作者
    pub author: String,
    /// 简介
    pub intro: String,
    /// 本地封面字节（从 cover.jpg 读取，缺失则为 None）
    pub cover: Option<Vec<u8>>,
    /// 目录页 URL（解析书页时从"小说目录"链接获取）
    pub toc_url: Option<String>,
    /// 卷列表
    pub volumes: Vec<Volume>,
    /// 已下载的图片表：键为文件名（如 "0_0_0.jpg"），值为字节
    pub images: HashMap<String, Vec<u8>>,
}

impl Book {
    /// 遍历所有章节，返回 (卷序号, 章序号, 章节引用)
    pub fn chapters(&self) -> impl Iterator<Item = (usize, usize, &Chapter)> {
        self.volumes
            .iter()
            .enumerate()
            .flat_map(|(vi, vol)| vol.chapters.iter().enumerate().map(move |(ci, ch)| (vi, ci, ch)))
    }
}

/// 一卷
#[derive(Debug, Default)]
pub struct Volume {
    /// 卷名
    pub name: String,
    /// 章节列表
    pub chapters: Vec<Chapter>,
}

/// 一章
#[derive(Debug, Default)]
pub struct Chapter {
    /// 章节标题
    pub title: String,
    /// 章节页面 URL
    pub url: String,
    /// 解析后生成的 XHTML 内容（下载完成后填充）
    pub xhtml: Option<String>,
}

/// 下载器配置，由调用方注入，不硬编码
#[derive(Debug, Clone)]
pub struct DownloaderConfig {
    /// 图片并发数
    pub image_concurrency: usize,
    /// 是否启用请求延迟（防限流）
    pub delay_enabled: bool,
    /// 请求重试次数
    pub max_retries: u32,
    /// User-Agent 列表，请求时随机挑选
    pub user_agents: Vec<String>,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            image_concurrency: 3,
            delay_enabled: false,
            max_retries: 3,
            user_agents: vec!["Mozilla/5.0 (Windows NT 10.0; Win64; x64)".into()],
        }
    }
}
