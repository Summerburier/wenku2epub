use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::error::{Error, ErrorKind, Result};
use crate::model::{Book, Chapter, Volume};

/// 书名解析结果：按括号拆出三种格式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleParts {
    /// 完整书名（保留括号）
    pub full: String,
    /// 括号内的翻译名（无括号则为 None）
    pub in_bracket: Option<String>,
    /// 括号前的部分（无括号则为 None）
    pub before_bracket: Option<String>,
}

/// 解析书名结构，如 "最强废渣皇子暗中活跃于帝位之争(最强出涸皇子的暗跃帝位争夺)"。
/// 支持中文括号（）与英文括号()；只取第一对括号；括号为空视为无括号。
pub fn parse_title(title: &str) -> TitleParts {
    let full = title.to_string();
    for (open, close) in [('（', '）'), ('(', ')')] {
        if let Some(start) = title.find(open) {
            let after = &title[start + open.len_utf8()..];
            if let Some(end) = after.find(close) {
                let inner = after[..end].trim();
                if !inner.is_empty() {
                    let before = title[..start].trim();
                    return TitleParts {
                        full,
                        in_bracket: Some(inner.to_string()),
                        before_bracket: (!before.is_empty()).then(|| before.to_string()),
                    };
                }
            }
        }
    }
    TitleParts {
        full,
        in_bracket: None,
        before_bracket: None,
    }
}

/// 按书名格式选择应用书名
pub fn apply_title_style(parts: &TitleParts, style: crate::model::TitleStyle) -> String {
    match style {
        crate::model::TitleStyle::Full => parts.full.clone(),
        crate::model::TitleStyle::InBracket => parts
            .in_bracket
            .clone()
            .unwrap_or_else(|| parts.full.clone()),
        crate::model::TitleStyle::BeforeBracket => parts
            .before_bracket
            .clone()
            .unwrap_or_else(|| parts.full.clone()),
    }
}

/// 递归收集元素内文本，<br> 转成换行，保留段落结构
fn element_text_with_breaks(el: ElementRef) -> String {
    let mut out = String::new();
    for node in el.children() {
        if let Some(text) = node.value().as_text() {
            out.push_str(text.to_string().as_str());
        } else if let Some(child) = ElementRef::wrap(node) {
            if child.value().name() == "br" {
                out.push('\n');
            } else {
                out.push_str(&element_text_with_breaks(child));
            }
        }
    }
    // 源码换行与 <br> 叠加会产生空行：压缩连续换行、清理每行首尾空白
    out.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析小说基本信息
/// # 参数
/// - `html`：书页 HTML
/// - `url`：书页 URL，用于解析目录链接
/// - `book`：输出 Book 结构体
pub fn parse_book_info(html: &str, url: &str, book: &mut Book) -> Result<()> {
    let document = Html::parse_document(html);

    // 解析书名和作者
    let title_sel = Selector::parse("title").expect("解析章节错误");
    if let Some(title) = document.select(&title_sel).next() {
        let text = title.text().collect::<String>();
        let parts: Vec<&str> = text.split(" - ").collect();
        if parts.len() >= 3 {
            book.title = parts[0].trim().to_string();
            book.author = parts[1].trim().to_string();
            book.library = parts[2].trim().to_string();
        } else {
            book.title = text.trim().to_string();
        }
    }

    // 解析内容简介
    let hottext_sel = Selector::parse("span.hottext").expect("解析章节错误");
    let mut intro_text = String::new();
    'outer: for hot in document.select(&hottext_sel) {
        let label = hot.text().collect::<String>();
        if label.contains("内容简介") {
            for sibling in hot.next_siblings() {
                if let Some(el) = ElementRef::wrap(sibling) {
                    intro_text.push_str(&element_text_with_breaks(el));
                    intro_text.push('\n');
                }
            }
            break 'outer;
        }
    }
    book.intro = intro_text.trim().to_string();

    // 目录链接：文本为"小说目录"的 <a>
    let a_sel = Selector::parse("a").expect("解析链接错误");
    for a in document.select(&a_sel) {
        let text = a.text().collect::<String>().trim().to_string();
        if text == "小说目录" {
            if let Some(href) = a.value().attr("href") {
                let base = Url::parse(url)
                    .map_err(|e| Error::new(ErrorKind::Parse, format!("书页 URL 解析失败: {e}")))?;
                let abs = base
                    .join(href)
                    .map_err(|e| Error::new(ErrorKind::Parse, format!("目录链接解析失败: {e}")))?;
                book.toc_url = Some(abs.to_string());
                break;
            }
        }
    }

    // 封面 URL：wenku8 书页封面的真实结构为
    //   <img src="http://img.wenku8.com/image/2/2626/2626s.jpg" width="168" ...>
    // 封面是宽 168 的大图；下方"相关推荐"是宽 90 的小图。取 img.wenku8.com 下宽度最大者。
    let base = Url::parse(url)
        .map_err(|e| Error::new(ErrorKind::Parse, format!("书页 URL 解析失败: {e}")))?;
    let img_sel = Selector::parse("img[src*='img.wenku8.com/image/']").expect("解析链接错误");
    let mut best: Option<(u32, String)> = None;
    for img in document.select(&img_sel) {
        let width = img
            .value()
            .attr("width")
            .and_then(|w| w.parse::<u32>().ok())
            .unwrap_or(0);
        let Some(src) = img.value().attr("src") else {
            continue;
        };
        let Ok(abs) = base.join(src) else {
            continue;
        };
        if best.as_ref().map_or(true, |(best_w, _)| width > *best_w) {
            best = Some((width, abs.to_string()));
        }
    }
    if let Some((_, url)) = best {
        book.cover_url = Some(url);
    }

    if book.title.is_empty() {
        return Err(Error::new(
            ErrorKind::Parse,
            "未能获取书名，可能网址无效或页面结构变化".to_string(),
        ));
    }
    Ok(())
}

/// 解析目录页：卷/章节/链接
pub fn parse_toc(html: &str, toc_url: &str, book: &mut Book) -> Result<()> {
    let document = Html::parse_document(html);
    let base = Url::parse(toc_url)
        .map_err(|e| Error::new(ErrorKind::Parse, format!("目录页 URL 解析失败: {e}")))?;

    let td_sel = Selector::parse("td").expect("解析目录错误");
    let a_sel = Selector::parse("a").expect("解析链接错误");

    let mut current_volume: Option<usize> = None;

    for td in document.select(&td_sel) {
        let classes = td.value().classes().collect::<Vec<_>>();

        // 卷标题行
        if classes.contains(&"vcss") {
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
            continue;
        }

        // 章节行
        if classes.contains(&"ccss") {
            let link = td
                .select(&a_sel)
                .next()
                .map(|a| (a.text().collect::<String>().trim().to_string(), a.value().attr("href")));

            if let (Some(vi), Some((title, Some(href)))) = (current_volume, link) {
                let abs = base.join(href).map_err(|e| {
                    Error::new(ErrorKind::Parse, format!("章节链接解析失败: {e}"))
                })?;
                book.volumes[vi].chapters.push(Chapter {
                    title: title.trim().to_string(),
                    url: abs.to_string(),
                });
            }
        }
    }

    if book.volumes.is_empty() {
        return Err(Error::new(
            ErrorKind::Parse,
            "目录解析失败：未找到任何卷".to_string(),
        ));
    }
    Ok(())
}

/// 解析章节页，返回 (正文段落, 图片 URL 列表)。
/// 图片不在此处下载，由调用方统一处理（命名并写入 book.images）。
pub fn parse_chapter(html: &str, url: &str) -> Result<(Vec<String>, Vec<String>)> {
    let document = Html::parse_document(html);
    let content_sel = Selector::parse("#content").expect("解析章节错误");
    let base = Url::parse(url)
        .map_err(|e| Error::new(ErrorKind::Parse, format!("章节页 URL 解析失败: {e}")))?;

    let Some(content) = document.select(&content_sel).next() else {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("未找到 #content 内容区: {url}"),
        ));
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

    // 图片：收集 <img src> 并转为绝对 URL
    let img_sel = Selector::parse("img").expect("解析链接错误");
    let mut images = Vec::new();
    for img in content.select(&img_sel) {
        if let Some(src) = img.value().attr("src") {
            let abs = base
                .join(src)
                .map_err(|e| Error::new(ErrorKind::Parse, format!("图片链接解析失败: {e}")))?;
            images.push(abs.to_string());
        }
    }

    Ok((paragraphs, images))
}
