use std::collections::HashMap;
#[derive(Debug,Default)]
pub struct Book {
    /// 书名
    pub title: String,
    /// 作者
    pub author: String,
    /// 文库
    pub library: String,
    /// 简介
    pub intro: String,
    /// 封面
    pub cover: Option<Vec<u8>>,
    /// 目录页 URL
    pub toc_url: Option<String>,
    /// 卷列表
    pub volumes: Vec<Volume>,
    /// 待下载图片：文件名->URL
    pub images: HashMap<String,String>,
}

#[derive(Debug,Default)]
pub struct Volume {
    /// 卷名
    pub name: String,
    /// 章节列表
    pub chapters: Vec<Chapter>,
}

#[derive(Debug,Default)]
pub struct Chapter {
    /// 章节名
    pub title: String,
    /// 章节 URL
    pub url: String,
}

