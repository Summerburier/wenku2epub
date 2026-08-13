use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::error::Result;
use crate::event::{Event, EventEmitter};
use crate::model::{Book, DownloaderConfig};

/// 下载器：流程编排，纯逻辑层，不依赖任何绑定层/UI 代码
pub struct Downloader {
    /// 注入的配置
    config: DownloaderConfig,
    /// 共享 HTTP 客户端（复用连接池，避免每请求重建）
    client: Arc<reqwest::Client>,
    /// 进度事件回调
    event_callback: Option<EventEmitter>,
    /// 全局图片计数器（跨卷章递增命名）
    img_counter: AtomicU64,
    /// 取消信号
    cancelled: Arc<AtomicBool>,
}

impl Downloader {
    /// 创建下载器，传入配置
    pub fn new(config: DownloaderConfig) -> Self {
        let client = Arc::new(
            crate::client::build_client().expect("创建 HTTP 客户端失败"),
        );
        Self {
            config,
            client,
            event_callback: None,
            img_counter: AtomicU64::new(0),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 绑定事件回调
    pub fn set_event_callback(&mut self, cb: EventEmitter) {
        self.event_callback = Some(cb);
    }

    /// 取消任务
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// 发射一个事件
    fn emit(&self, event: Event) {
        if let Some(cb) = &self.event_callback {
            cb(event);
        }
    }

    /// 执行完整下载流程，返回生成的 EPUB 路径
    ///
    /// 流程：解析书页 → 解析目录 → 逐章抓取正文+图片 → 打包 EPUB
    pub async fn run(&self, url: &str) -> Result<String> {
        self.cancelled.store(false, Ordering::SeqCst);
        self.emit(Event::StartBook);

        let mut book = Book::default();
        // 加载本地封面（若存在）
        if let Ok(bytes) = std::fs::read("cover.jpg") {
            book.cover = Some(bytes);
        }
        self.parse_book_info(url, &mut book).await?;
        self.parse_toc(&mut book).await?;

        // 逐章抓取正文和图片
        let chapter_semaphore = Arc::new(Semaphore::new(1));
        let image_semaphore = Arc::new(Semaphore::new(self.config.image_concurrency));
        let total = book.chapters().count();
        let mut current = 0usize;
        for (vi, ci, title, chapter_url) in {
            let mut chapters: Vec<(usize, usize, String, String)> = Vec::new();
            for (vi, ci, ch) in book.chapters() {
                chapters.push((vi, ci, ch.title.clone(), ch.url.clone()));
            }
            chapters
        } {
            self.check_cancelled()?;

            let _permit = chapter_semaphore.acquire().await;
            self.fetch_chapter(&title, &chapter_url, vi, ci, image_semaphore.clone(), &mut book).await?;
            current += 1;
            self.emit(Event::ChapterDone { current, total });
        }

        // 打包 EPUB
        let path = crate::epub::write_epub(&book)?;
        self.emit(Event::BookDone { path: path.clone() });
        Ok(path)
    }

    /// 解析书页：书名/作者/简介/目录链接
    async fn parse_book_info(&self, url: &str, book: &mut Book) -> Result<()> {
        let html = crate::client::fetch(&self.client, &self.config, url).await?;
        crate::parser::parse_book_info(&html, url, book)
    }

    /// 解析目录：卷/章节/链接
    async fn parse_toc(&self, book: &mut Book) -> Result<()> {
        self.emit(Event::StartParseToc);

        let toc_url = book
            .toc_url
            .clone()
            .ok_or(crate::error::DownloadError::NotFound("未找到目录链接".into()))?;

        let html = crate::client::fetch(&self.client, &self.config, &toc_url).await?;
        crate::parser::parse_toc(&html, &toc_url, book)?;

        let volumes = book.volumes.len();
        let chapters = book.volumes.iter().map(|v| v.chapters.len()).sum();
        self.emit(Event::ParseTocDone { volumes, chapters });
        Ok(())
    }

    /// 抓取单章正文 + 图片，生成 XHTML 存入模型
    ///
    /// 图片用信号量限制并发，真正并行下载（复用共享 Client 的连接池）
    async fn fetch_chapter(
        &self,
        title: &str,
        chapter_url: &str,
        vi: usize,
        ci: usize,
        image_semaphore: Arc<Semaphore>,
        book: &mut Book,
    ) -> Result<()> {
        let html = crate::client::fetch(&self.client, &self.config, chapter_url).await?;
        let (text, images) = crate::parser::parse_chapter(&html, chapter_url)?;

        // 并发下载图片，命名 {volume}_{chapter}_{序号}.jpg
        let total_images = images.len();
        let mut image_names = vec![String::new(); total_images];
        let mut results = Vec::with_capacity(total_images);

        for (j, src) in images.iter().enumerate() {
            let permit = image_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| crate::error::DownloadError::Http(e.to_string()))?;
            let client = self.client.clone();
            let config = self.config.clone();
            let src = src.clone();
            let imgname = format!("{vi}_{ci}_{j}.jpg");
            results.push(tokio::spawn(async move {
                // permit 在整个下载期间持有，实现并发上限
                let _guard = permit;
                let bytes = crate::client::download_bytes(&client, &config, &src).await;
                (j, imgname, bytes)
            }));
        }

        for handle in results {
            let (j, imgname, bytes) = handle
                .await
                .map_err(|e| crate::error::DownloadError::Http(e.to_string()))?;
            let bytes = bytes?;
            book.images.insert(imgname.clone(), bytes);
            image_names[j] = imgname;
            self.emit(Event::ImageDone { current: j + 1, total: total_images });
        }

        // 生成 XHTML（正文段落 + 图片引用）
        let xhtml = crate::epub::build_chapter_xhtml(title, &text, &image_names);
        if let Some(ch) = book.volumes.get_mut(vi).and_then(|v| v.chapters.get_mut(ci)) {
            ch.xhtml = Some(xhtml);
        }
        Ok(())
    }

    /// 检查是否被取消
    fn check_cancelled(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(crate::error::DownloadError::Cancelled);
        }
        Ok(())
    }
}
