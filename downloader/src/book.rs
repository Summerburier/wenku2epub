use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use reqwest::Client;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::client::{fetch_bytes, fetch_html};
use crate::cover::{resolve_cover, CoverSource};
use crate::error::{Error, ErrorKind, Result};
use crate::model::{escape_xml, sanitize_filename, Book, Chapter, EpubFile, Progress, Selection, Stage};
use crate::parser::{parse_book_info, parse_chapter, parse_toc};
use crate::{gen_v2, gen_v3};

/// EPUB 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpubVersion {
    V2,
    V3,
}

/// 一本书的下载结果
#[derive(Debug)]
pub struct BookResult {
    /// 生成的 EPUB 文件路径
    pub path: String,
    /// 下载失败的图片数
    pub failed_images: usize,
}

/// 生成单章 XHTML（图片用文件名占位）
fn build_chapter_xhtml(title: &str, paragraphs: &[String], image_names: &[String]) -> String {
    let mut body = String::new();
    body.push_str(&format!("<h3>{}</h3>\n", escape_xml(title)));
    for p in paragraphs {
        body.push_str(&format!("<p>{}</p>\n", escape_xml(p)));
    }
    for img in image_names {
        body.push_str(&format!(
            "<img src=\"../Image/{}\" alt=\"\" />\n",
            escape_xml(img)
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>{title}</title>
</head>
<body>
  <section>
{body}  </section>
</body>
</html>
"#,
        title = escape_xml(title),
    )
}

/// 按选择范围过滤章节
fn apply_selection(mut book: Book, selection: &Selection) -> Result<Book> {
    match selection {
        Selection::All => {}
        Selection::Range { start, end } => {
            if *start == 0 || end < start {
                return Err(Error::new(ErrorKind::NotFound, "无效的章节区间".into()));
            }
            let mut kept = Vec::new();
            let mut idx = 1usize;
            for mut vol in book.volumes.drain(..) {
                vol.chapters = vol
                    .chapters
                    .drain(..)
                    .filter(|_| {
                        let keep = idx >= *start && idx <= *end;
                        idx += 1;
                        keep
                    })
                    .collect();
                if !vol.chapters.is_empty() {
                    kept.push(vol);
                }
            }
            book.volumes = kept;
        }
        Selection::Chapters(ids) => {
            let mut kept = Vec::new();
            let mut idx = 1usize;
            for mut vol in book.volumes.drain(..) {
                vol.chapters = vol
                    .chapters
                    .drain(..)
                    .filter(|_| {
                        let keep = ids.contains(&idx);
                        idx += 1;
                        keep
                    })
                    .collect();
                if !vol.chapters.is_empty() {
                    kept.push(vol);
                }
            }
            book.volumes = kept;
        }
    }

    if book.volumes.is_empty() {
        return Err(Error::new(
            ErrorKind::NotFound,
            "选择范围后没有可下载的章节".into(),
        ));
    }
    Ok(book)
}

/// 打包 EPUB 文件清单为 zip 字节（mimetype 必须第一个且 STORE）
fn pack_epub(files: &[EpubFile]) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));

    for (i, f) in files.iter().enumerate() {
        let options = if i == 0 {
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
        } else {
            zip::write::SimpleFileOptions::default()
        };
        writer
            .start_file(&f.path, options)
            .map_err(|e| Error::new(ErrorKind::Encode, format!("zip 写入失败: {e}")))?;
        writer
            .write_all(&f.bytes)
            .map_err(|e| Error::new(ErrorKind::Encode, format!("zip 写入失败: {e}")))?;
    }

    let cursor = writer
        .finish()
        .map_err(|e| Error::new(ErrorKind::Encode, format!("zip 收尾失败: {e}")))?;
    Ok(cursor.into_inner())
}

/// 一本书的完整流水线：解析 → 过滤 → 下载章节 → 下载图片 → 打包
pub async fn generate_book(
    client: &Client,
    url: &str,
    selection: &Selection,
    concurrency: usize,
    image_concurrency: usize,
    version: EpubVersion,
    title_style: crate::model::TitleStyle,
    cover_source: CoverSource,
    progress: &Arc<Progress>,
) -> Result<BookResult> {
    progress.set_stage(Stage::FetchBook);

    // 1. 解析书页信息
    let html = fetch_html(client, url).await?;
    let mut book = Book::default();
    parse_book_info(&html, url, &mut book)?;
    book.title = crate::parser::apply_title_style(
        &crate::parser::parse_title(&book.title),
        title_style,
    );

    // 2. 解析目录
    progress.set_stage(Stage::ParseToc);
    let toc_url = book
        .toc_url
        .clone()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "未找到目录链接".into()))?;
    let toc_html = fetch_html(client, &toc_url).await?;
    parse_toc(&toc_html, &toc_url, &mut book)?;

    // 3. 按选择范围过滤
    book = apply_selection(book, selection)?;

    // 4. 并发下载并解析章节
    progress.set_stage(Stage::DownloadChapters);
    let mut chapters: Vec<(usize, usize, Chapter)> = Vec::new();
    for (vi, vol) in book.volumes.iter().enumerate() {
        for (ci, ch) in vol.chapters.iter().enumerate() {
            chapters.push((vi, ci, ch.clone()));
        }
    }
    progress
        .chapters_total
        .store(chapters.len(), Ordering::Relaxed);

    let sem = Arc::new(Semaphore::new(concurrency));
    let mut tasks = JoinSet::new();
    let mut chapter_files = Vec::new();
    let mut all_images: HashMap<String, String> = HashMap::new();

    for (vi, ci, ch) in chapters {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(Error::new(ErrorKind::Cancelled, "用户取消下载".into()));
        }
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| Error::new(ErrorKind::Network, format!("获取并发许可失败: {e}")))?;
        let c = client.clone();
        let u = ch.url.clone();
        let t = ch.title.clone();
        let prog = progress.clone();
        tasks.spawn(async move {
            let _permit = permit;
            let result: Result<(String, Vec<(String, String)>)> = async {
                let chap_html = fetch_html(&c, &u).await?;
                let (paragraphs, image_urls) = parse_chapter(&chap_html, &u)?;
                let mut imgs = Vec::new();
                for (j, src) in image_urls.iter().enumerate() {
                    imgs.push((format!("{vi}_{ci}_{j}.jpg"), src.clone()));
                }
                let names: Vec<String> = imgs.iter().map(|(n, _)| n.clone()).collect();
                let xhtml = build_chapter_xhtml(&t, &paragraphs, &names);
                Ok((xhtml, imgs))
            }
            .await;
            prog.chapters_done.fetch_add(1, Ordering::Relaxed);
            (vi, ci, result)
        });
    }

    while let Some(joined) = tasks.join_next().await {
        let (vi, ci, result) = joined
            .map_err(|e| Error::new(ErrorKind::Encode, format!("章节任务失败: {e}")))?;
        let (xhtml, imgs) = result?;
        chapter_files.push(EpubFile {
            path: format!("OEBPS/Text/{vi}_{ci}.xhtml"),
            bytes: xhtml.into_bytes(),
        });
        for (name, src) in imgs {
            all_images.insert(name, src);
        }
    }

    // 5. 并发下载图片
    progress.set_stage(Stage::DownloadImages);
    progress
        .images_total
        .store(all_images.len(), Ordering::Relaxed);

    let sem = Arc::new(Semaphore::new(image_concurrency));
    let mut tasks = JoinSet::new();
    let mut downloaded: HashMap<String, Vec<u8>> = HashMap::new();
    let mut failed_images = 0usize;

    for (name, src) in &all_images {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(Error::new(ErrorKind::Cancelled, "用户取消下载".into()));
        }
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| Error::new(ErrorKind::Network, format!("获取并发许可失败: {e}")))?;
        let c = client.clone();
        let u = src.clone();
        let n = name.clone();
        let prog = progress.clone();
        tasks.spawn(async move {
            let _permit = permit;
            let result = fetch_bytes(&c, &u).await;
            prog.images_done.fetch_add(1, Ordering::Relaxed);
            (n, result)
        });
    }

    while let Some(joined) = tasks.join_next().await {
        let (name, result) = joined
            .map_err(|e| Error::new(ErrorKind::Encode, format!("图片任务失败: {e}")))?;
        match result {
            Ok(bytes) => {
                downloaded.insert(name, bytes);
            }
            Err(e) => {
                failed_images += 1;
                eprintln!("图片下载失败（已跳过）：{} {}", name, e);
            }
        }
    }

    // 6. 获取封面（按策略，失败则跳过不视为错误）
    progress.set_stage(Stage::Pack);
    book.cover = resolve_cover(client, &book, &downloaded, cover_source).await?;

    let mut files = Vec::new();
    files.push(EpubFile {
        path: "mimetype".into(),
        bytes: b"application/epub+zip".to_vec(),
    });
    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
   </rootfiles>
</container>
"#;
    files.push(EpubFile {
        path: "META-INF/container.xml".into(),
        bytes: container_xml.as_bytes().to_vec(),
    });

    match version {
        EpubVersion::V3 => files.extend(gen_v3::generate(&book)?),
        EpubVersion::V2 => files.extend(gen_v2::generate(&book)?),
    }
    files.extend(chapter_files);
    for (name, bytes) in &downloaded {
        files.push(EpubFile {
            path: format!("OEBPS/Image/{name}"),
            bytes: bytes.clone(),
        });
    }
    if let Some(cover) = &book.cover {
        files.push(EpubFile {
            path: "OEBPS/Image/cover.jpg".into(),
            bytes: cover.clone(),
        });
    }

    let zip_bytes = pack_epub(&files)?;
    let filename = sanitize_filename(&book.title);
    let path = format!("{filename}.epub");
    std::fs::write(&path, zip_bytes)
        .map_err(|e| Error::new(ErrorKind::Encode, format!("写入文件失败: {e}")))?;

    Ok(BookResult {
        path,
        failed_images,
    })
}
