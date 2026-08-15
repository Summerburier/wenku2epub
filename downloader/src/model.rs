use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

#[derive(Debug, Default)]
pub struct Book {
    /// 书名
    pub title: String,
    /// 作者
    pub author: String,
    /// 文库
    pub library: String,
    /// 简介
    pub intro: String,
    /// 封面字节（已获取）
    pub cover: Option<Vec<u8>>,
    /// 封面 URL（书页解析所得）
    pub cover_url: Option<String>,
    /// 目录页 URL
    pub toc_url: Option<String>,
    /// 卷列表
    pub volumes: Vec<Volume>,
    /// 待下载图片：文件名->URL
    pub images: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct Volume {
    /// 卷名
    pub name: String,
    /// 章节列表
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Default, Clone)]
pub struct Chapter {
    /// 章节名
    pub title: String,
    /// 章节 URL
    pub url: String,
}

/// 下载范围选择：全书 / 区间 / 指定章节
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    All,
    Range { start: usize, end: usize },
    Chapters(Vec<usize>),
}

impl Default for Selection {
    fn default() -> Self {
        Self::All
    }
}

/// EPUB 包内的一个文件条目
#[derive(Debug, Clone)]
pub struct EpubFile {
    /// 包内路径，如 "OEBPS/Text/0_0.xhtml"
    pub path: String,
    /// 文件字节
    pub bytes: Vec<u8>,
}

/// 下载阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    FetchBook,
    ParseToc,
    DownloadChapters,
    DownloadImages,
    Pack,
}

impl From<u8> for Stage {
    fn from(v: u8) -> Self {
        match v {
            0 => Stage::FetchBook,
            1 => Stage::ParseToc,
            2 => Stage::DownloadChapters,
            3 => Stage::DownloadImages,
            _ => Stage::Pack,
        }
    }
}

/// 一本书的共享进度（原子无锁，供前端轮询）
#[derive(Debug, Default)]
pub struct Progress {
    pub chapters_done: AtomicUsize,
    pub chapters_total: AtomicUsize,
    pub images_done: AtomicUsize,
    pub images_total: AtomicUsize,
    pub cancel: AtomicBool,
    stage: AtomicU8,
}

impl Progress {
    pub fn set_stage(&self, stage: Stage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }

    pub fn get_stage(&self) -> Stage {
        Stage::from(self.stage.load(Ordering::Relaxed))
    }
}

/// XML 转义
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 清理非法文件名字符
pub fn sanitize_filename(title: &str) -> String {
    title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
