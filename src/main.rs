mod client;
mod downloader;
mod epub;
mod error;
mod event;
mod model;
mod parser;

use std::sync::{Arc, Mutex};

use dialoguer::{Confirm, Input};
use downloader::Downloader;
use event::{DownloaderEvent, Event};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use model::DownloaderConfig;

/// CLI 展示层：实现事件 trait，用 indicatif 进度条展示进度
struct Cli {
    /// 多进度条管理器
    mp: MultiProgress,
    /// 章节进度条
    chapter_bar: Mutex<Option<ProgressBar>>,
    /// 当前章节的图片进度条
    image_bar: Mutex<Option<ProgressBar>>,
}

impl Cli {
    fn new() -> Self {
        Self {
            mp: MultiProgress::new(),
            chapter_bar: Mutex::new(None),
            image_bar: Mutex::new(None),
        }
    }
}

impl DownloaderEvent for Cli {
    fn on_event(&self, event: Event) {
        match event {
            Event::StartBook => println!("开始下载..."),
            Event::StartParseToc => println!("开始解析目录..."),
            Event::ParseTocDone { chapters, .. } => {
                // 创建章节进度条
                let bar = self.mp.add(ProgressBar::new(chapters as u64));
                bar.set_style(
                    ProgressStyle::with_template(
                        "{msg} [{bar:40}] {pos}/{len} 章",
                    )
                    .unwrap()
                    .progress_chars("=>-"),
                );
                bar.set_message("处理章节");
                *self.chapter_bar.lock().unwrap() = Some(bar);
            }
            Event::ChapterDone { current, .. } => {
                if let Some(bar) = self.chapter_bar.lock().unwrap().as_ref() {
                    bar.set_position(current as u64);
                }
                // 移除当前章节的图片进度条
                if let Some(img_bar) = self.image_bar.lock().unwrap().take() {
                    img_bar.finish_and_clear();
                    self.mp.remove(&img_bar);
                }
            }
            Event::ImageDone { current, total } => {
                let mut img_lock = self.image_bar.lock().unwrap();
                if img_lock.is_none() {
                    // 当前章节的第一张图，创建图片进度条
                    let bar = self.mp.insert(0, ProgressBar::new(total as u64));
                    bar.set_style(
                        ProgressStyle::with_template(
                            "  {msg} [{bar:30}] {pos}/{len} 图",
                        )
                        .unwrap()
                        .progress_chars("=>-"),
                    );
                    bar.set_message("图片");
                    *img_lock = Some(bar);
                }
                if let Some(bar) = img_lock.as_ref() {
                    bar.set_position(current as u64);
                }
            }
            Event::BookDone { path } => {
                println!("EPUB 已生成：{path}");
            }
            Event::Error { message } => println!("错误：{message}"),
        }
    }
}

#[tokio::main]
async fn main() {
    let url: String = Input::new()
        .with_prompt("请输入要下载的小说网址")
        .interact_text()
        .expect("读取网址失败");

    let delay_enabled = Confirm::new()
        .with_prompt("是否启用请求延迟以防止报错?")
        .default(false)
        .interact()
        .expect("读取延迟选项失败");

    let image_concurrency: usize = Input::new()
        .with_prompt("图片并发数(默认 3)")
        .default(3)
        .interact_text()
        .expect("读取并发数失败");

    let config = DownloaderConfig {
        image_concurrency,
        delay_enabled,
        ..Default::default()
    };

    let mut dl = Downloader::new(config);
    dl.set_event_callback(event::emitter_from(Arc::new(Cli::new())));

    match dl.run(&url).await {
        Ok(path) => println!("成功：{path}"),
        Err(e) => println!("失败：{e}"),
    }
}
