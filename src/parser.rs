use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::error::{DownloadError, Result};
use crate::model::{Book, Chapter, Volume};

/// 解析书页：书名/作者/简介/目录链接
///
/// 对照 JS 版 getBookInfo：
/// - 书名/作者从 <title> 的 "xxx - xxx - xxx - xxx" 格式提取
/// - 简介从 span[style="font-size:14px;"] 提取
/// - 目录链接从文本为"小说目录"的 <a> 提取，并解析为绝对地址
pub fn parse_book_info(html: &str, url: &str, book: &mut Book) -> Result<()> {
    let document = Html::parse_document(html);

    // 书名/作者：<title> 文本形如 "书名 - 作者 - ..."
    if let Some(title) = document.select(&Selector::parse("title").unwrap()).next() {
        let text = title.text().collect::<String>();
        let parts: Vec<&str> = text.split(" - ").collect();
        if parts.len() >= 2 {
            book.title = parts[0].trim().to_string();
            book.author = parts[1].trim().to_string();
        } else {
            book.title = text.trim().to_string();
        }
    }

    // 简介：位于"内容简介："标识之后的 font-size:14px span
    // wenku8 页面上有多个 14px span（如"最近章节"），不能全页面遍历取最后一个。
    // 可靠方式：找到文本含"内容简介"的 hottext 元素，取其下一个兄弟元素。
    let hottext_sel = Selector::parse("span.hottext").unwrap();
    let mut intro_text = String::new();
    'outer: for hot in document.select(&hottext_sel) {
        let label = hot.text().collect::<String>();
        if label.contains("内容简介") {
            // 取该元素之后的所有兄弟节点，拼接文本
            for sibling in hot.next_siblings() {
                if let Some(el) = ElementRef::wrap(sibling) {
                    intro_text.push_str(&el.text().collect::<String>());
                    intro_text.push('\n');
                }
            }
            break 'outer;
        }
    }
    book.intro = intro_text.trim().to_string();
    if book.intro.is_empty() {
        book.intro = "暂无简介".to_string();
    }

    // 目录链接：文本为"小说目录"的 <a>
    let a_sel = Selector::parse("a").unwrap();
    for a in document.select(&a_sel) {
        let text = a.text().collect::<String>().trim().to_string();
        if text == "小说目录" {
            if let Some(href) = a.value().attr("href") {
                if let Ok(abs) = Url::parse(url).and_then(|base| base.join(href)) {
                    book.toc_url = Some(abs.to_string());
                    break;
                }
            }
        }
    }

    if book.title.is_empty() {
        return Err(DownloadError::Parse("未能获取书名，可能网址无效或页面结构变化".into()));
    }
    Ok(())
}

/// 解析目录页：卷/章节/链接
///
/// 对照 JS 版 getChapList：
/// - td.vcss → 卷名，新建一个卷
/// - td.ccss 下第一个 <a> → 章节标题 + 跳转链接
pub fn parse_toc(html: &str, toc_url: &str, book: &mut Book) -> Result<()> {
    let document = Html::parse_document(html);
    let base = Url::parse(toc_url).map_err(|e| DownloadError::Parse(e.to_string()))?;

    let td_sel = Selector::parse("td").unwrap();
    let a_sel = Selector::parse("a").unwrap();
    let vcss_sel = Selector::parse("td.vcss").unwrap();
    let ccss_a_sel = Selector::parse("td.ccss a").unwrap();

    let mut current_volume: Option<usize> = None;
    let mut current_chapter: Option<usize> = None;

    for td in document.select(&td_sel) {
        let classes = td.value().classes().collect::<Vec<_>>();

        if classes.contains(&"vcss") {
            // 卷：textContent 里的卷名
            let vol_name = td
                .text()
                .map(|s| s.trim())
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string();
            book.volumes.push(Volume {
                name: vol_name,
                chapters: Vec::new(),
            });
            current_volume = Some(book.volumes.len() - 1);
            current_chapter = None;
            continue;
        }

        if classes.contains(&"ccss") {
            // 章节：td.ccss 下的第一个 a
            let link = td
                .select(&a_sel)
                .next()
                .map(|a| (a.text().collect::<String>().trim().to_string(), a.value().attr("href")));

            if let (Some(vi), Some((title, Some(href)))) = (current_volume, link) {
                let abs = base
                    .join(href)
                    .map_err(|e| DownloadError::Parse(format!("章节链接解析失败: {e}")))?;
                let chapter = Chapter {
                    title: title.trim().to_string(),
                    url: abs.to_string(),
                    xhtml: None,
                };
                book.volumes[vi].chapters.push(chapter);
                current_chapter = Some(book.volumes[vi].chapters.len() - 1);
            }
        }

        let _ = current_chapter;
    }

    // 校验
    let _ = vcss_sel;
    let _ = ccss_a_sel;
    if book.volumes.is_empty() {
        return Err(DownloadError::Parse("目录解析失败：未找到任何卷".into()));
    }
    Ok(())
}

/// 解析章节页，返回 (正文段落, 图片 URL 列表)
///
/// 对照 JS 版 processChapter：
/// - 正文取 #content 下的直接文本节点
/// - 图片取 #content 下所有 <img> 的 src，解析为绝对地址
pub fn parse_chapter(html: &str, url: &str) -> Result<(Vec<String>, Vec<String>)> {
    let document = Html::parse_document(html);
    let content_sel = Selector::parse("#content").unwrap();
    let base = Url::parse(url).map_err(|e| DownloadError::Parse(e.to_string()))?;

    let Some(content) = document.select(&content_sel).next() else {
        return Err(DownloadError::NotFound("未找到 #content 内容区".into()));
    };

    // 正文段落：直接子文本节点
    let mut paragraphs = Vec::new();
    for node in content.children() {
        if let Some(text_value) = node.value().as_text() {
            let text = text_value.to_string().trim().to_string();
            if !text.is_empty() {
                paragraphs.push(text);
            }
        }
    }

    // 图片：所有 <img> 的 src
    let img_sel = Selector::parse("img").unwrap();
    let mut images = Vec::new();
    for img in content.select(&img_sel) {
        if let Some(src) = img.value().attr("src") {
            if let Ok(abs) = base.join(src) {
                images.push(abs.to_string());
            }
        }
    }

    Ok((paragraphs, images))
}
