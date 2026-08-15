use crate::error::Result;
use crate::model::{escape_xml, Book, EpubFile};

/// 生成 EPUB3 的 content.opf 和 nav.xhtml
pub fn generate(book: &Book) -> Result<Vec<EpubFile>> {
    let mut files = Vec::new();
    files.push(EpubFile {
        path: "OEBPS/content.opf".into(),
        bytes: build_opf(book).into_bytes(),
    });
    files.push(EpubFile {
        path: "OEBPS/nav.xhtml".into(),
        bytes: build_nav(book).into_bytes(),
    });
    Ok(files)
}

fn build_opf(book: &Book) -> String {
    let mut manifest = String::new();
    let mut spine = String::new();

    if book.cover.is_some() {
        manifest.push_str(
            r#"  <item id="cover" href="Image/cover.jpg" media-type="image/jpeg" properties="cover-image" />"#,
        );
        manifest.push('\n');
    }
    manifest.push_str(
        r#"  <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav" />"#,
    );
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
  <spine>
{spine}  </spine>
</package>
"#,
        title = escape_xml(&book.title),
        author = escape_xml(&book.author),
        intro = escape_xml(&book.intro),
    )
}

fn build_nav(book: &Book) -> String {
    let mut items = String::new();
    for (vi, vol) in book.volumes.iter().enumerate() {
        items.push_str(&format!(
            "    <li><a href=\"Text/{vi}_0.xhtml\">{}</a>\n",
            escape_xml(&vol.name)
        ));
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
