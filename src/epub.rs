use crate::error::{DownloadError, Result};
use crate::model::Book;

/// 生成单章的 XHTML 内容（正文段落 + 图片引用）
pub fn build_chapter_xhtml(title: &str, paragraphs: &[String], image_names: &[String]) -> String {
    let mut body = String::new();
    body.push_str(&format!("<h3>{}</h3>\n", escape_xml(title)));

    for p in paragraphs {
        body.push_str(&format!("<p>{}</p>\n", escape_xml(p)));
    }

    for img in image_names {
        body.push_str(&format!("<img src=\"../Image/{}\" alt=\"\" />\n", escape_attr(img)));
    }

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="../Style/style.css" />
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

/// 转义 XML 特殊字符
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 转义属性值
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

/// 将 Book 打包为 EPUB 文件，返回输出路径
pub fn write_epub(book: &Book) -> Result<String> {
    if book.title.is_empty() {
        return Err(DownloadError::NotFound("书名缺失，无法打包".into()));
    }

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // mimetype 必须 STORE（不压缩）且放第一个
    zip.start_file("mimetype", options)
        .map_err(|e| DownloadError::Parse(e.to_string()))?;
    std::io::Write::write_all(&mut zip, b"application/epub+zip")
        .map_err(|e| DownloadError::Parse(e.to_string()))?;

    // container.xml
    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
   </rootfiles>
</container>
"#;
    let deflate = zip::write::SimpleFileOptions::default();
    zip.start_file("META-INF/container.xml", deflate)
        .map_err(|e| DownloadError::Parse(e.to_string()))?;
    std::io::Write::write_all(&mut zip, container_xml.as_bytes())
        .map_err(|e| DownloadError::Parse(e.to_string()))?;

    // content.opf
    let opf = build_opf(book);
    zip.start_file("OEBPS/content.opf", deflate)
        .map_err(|e| DownloadError::Parse(e.to_string()))?;
    std::io::Write::write_all(&mut zip, opf.as_bytes())
        .map_err(|e| DownloadError::Parse(e.to_string()))?;

    // nav.xhtml
    let nav = build_nav(book);
    zip.start_file("OEBPS/nav.xhtml", deflate)
        .map_err(|e| DownloadError::Parse(e.to_string()))?;
    std::io::Write::write_all(&mut zip, nav.as_bytes())
        .map_err(|e| DownloadError::Parse(e.to_string()))?;

    // 章节 XHTML
    for (vi, vol) in book.volumes.iter().enumerate() {
        for (ci, ch) in vol.chapters.iter().enumerate() {
            if let Some(xhtml) = &ch.xhtml {
                let name = format!("OEBPS/Text/{vi}_{ci}.xhtml");
                zip.start_file(&name, deflate)
                    .map_err(|e| DownloadError::Parse(e.to_string()))?;
                std::io::Write::write_all(&mut zip, xhtml.as_bytes())
                    .map_err(|e| DownloadError::Parse(e.to_string()))?;
            }
        }
    }

    // 图片
    for (name, bytes) in &book.images {
        let path = format!("OEBPS/Image/{name}");
        zip.start_file(&path, deflate)
            .map_err(|e| DownloadError::Parse(e.to_string()))?;
        std::io::Write::write_all(&mut zip, bytes.as_slice())
            .map_err(|e| DownloadError::Parse(e.to_string()))?;
    }

    // 封面（若存在）
    if let Some(cover) = &book.cover {
        zip.start_file("OEBPS/Image/cover.jpg", deflate)
            .map_err(|e| DownloadError::Parse(e.to_string()))?;
        std::io::Write::write_all(&mut zip, cover.as_slice())
            .map_err(|e| DownloadError::Parse(e.to_string()))?;
    }

    let cursor = zip
        .finish()
        .map_err(|e| DownloadError::Parse(e.to_string()))?;
    let bytes = cursor.into_inner();

    let filename = sanitize_filename(&book.title);
    let path = format!("{filename}.epub");
    std::fs::write(&path, bytes).map_err(|e| DownloadError::Http(format!("写入文件失败: {e}")))?;

    Ok(path)
}

/// 生成 content.opf
fn build_opf(book: &Book) -> String {
    let mut manifest = String::new();
    let mut spine = String::new();

    manifest.push_str(r#"  <item id="cover" href="Image/cover.jpg" media-type="image/jpeg" properties="cover-image" />"#);
    manifest.push('\n');
    manifest.push_str(r#"  <item id="style" href="Style/style.css" media-type="text/css" />"#);
    manifest.push('\n');
    manifest.push_str(r#"  <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav" />"#);
    manifest.push('\n');

    for (vi, vol) in book.volumes.iter().enumerate() {
        for (ci, _) in vol.chapters.iter().enumerate() {
            let id = format!("Text/{vi}_{ci}.xhtml");
            manifest.push_str(&format!(
                "  <item id=\"{id}\" href=\"{id}\" media-type=\"application/xhtml+xml\" />\n"
            ));
            spine.push_str(&format!("    <itemref idref=\"{id}\" />\n"));
        }
    }

    for name in book.images.keys() {
        let id = format!("Image/{name}");
        manifest.push_str(&format!(
            "  <item id=\"{id}\" href=\"{id}\" media-type=\"image/jpeg\" />\n"
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="PrimaryID">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
    <dc:identifier opf:scheme="ISBN"/>
    <dc:language>zh-CN</dc:language>
    <dc:creator>{author}</dc:creator>
    <dc:description>{intro}</dc:description>
  </metadata>
  <manifest>
{manifest}  </manifest>
  <spine toc="ncx">
{spine}  </spine>
</package>
"#,
        title = escape_xml(&book.title),
        author = escape_xml(&book.author),
        intro = escape_xml(&book.intro),
    )
}

/// 生成 nav.xhtml 导航
fn build_nav(book: &Book) -> String {
    let mut items = String::new();
    for (vi, vol) in book.volumes.iter().enumerate() {
        items.push_str(&format!("    <li><a href=\"Text/{vi}_0.xhtml\">{}</a>\n", escape_xml(&vol.name)));
        items.push_str("      <ol>\n");
        for (ci, ch) in vol.chapters.iter().enumerate() {
            items.push_str(&format!(
                "        <li><a href=\"Text/{vi}_{ci}.xhtml\">{}</a></li>\n",
                escape_xml(&ch.title)
            ));
        }
        items.push_str("      </ol>\n    </li>\n");
    }

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="zh-CN" xml:lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <title>ePub Nav</title>
    <style type="text/css">
      ol {{ list-style-type: none; margin: 0; padding: 0; }}
      li {{ margin: 0.2em 0; }}
    </style>
  </head>
  <body epub:type="frontmatter">
    <nav epub:type="toc" id="toc">
      <ol>
{items}      </ol>
    </nav>
  </body>
</html>
"#,
    )
}

/// 清理非法文件名字符
fn sanitize_filename(title: &str) -> String {
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
