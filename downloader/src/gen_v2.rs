use crate::error::Result;
use crate::model::{escape_xml, Book, EpubFile};

/// 生成 EPUB2 的 content.opf 和 toc.ncx
pub fn generate(book: &Book) -> Result<Vec<EpubFile>> {
    let mut files = Vec::new();
    files.push(EpubFile {
        path: "OEBPS/content.opf".into(),
        bytes: build_opf(book).into_bytes(),
    });
    files.push(EpubFile {
        path: "OEBPS/toc.ncx".into(),
        bytes: build_ncx(book).into_bytes(),
    });
    Ok(files)
}

fn build_opf(book: &Book) -> String {
    let mut manifest = String::new();
    let mut spine = String::new();

    if book.cover.is_some() {
        manifest.push_str(
            r#"  <item id="cover" href="Image/cover.jpg" media-type="image/jpeg" />"#,
        );
        manifest.push('\n');
    }
    manifest.push_str(r#"  <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml" />"#);
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

    let publisher = if book.library.is_empty() {
        String::new()
    } else {
        format!("    <dc:publisher>{}</dc:publisher>\n", escape_xml(&book.library))
    };

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="PrimaryID">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
    <dc:identifier opf:scheme="ISBN"/>
    <dc:language>zh-CN</dc:language>
    <dc:creator>{author}</dc:creator>
{publisher}    <dc:description>{intro}</dc:description>
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

fn build_ncx(book: &Book) -> String {
    let mut nav_map = String::new();
    let mut play_order = 0usize;

    for (vi, vol) in book.volumes.iter().enumerate() {
        play_order += 1;
        nav_map.push_str(&format!(
            r#"    <navPoint id="v{vi}" playOrder="{play_order}">
      <navLabel><text>{vol_name}</text></navLabel>
      <content src="Text/{vi}_0.xhtml" />
"#,
            vol_name = escape_xml(&vol.name),
        ));
        for (ci, ch) in vol.chapters.iter().enumerate() {
            play_order += 1;
            nav_map.push_str(&format!(
                r#"      <navPoint id="v{vi}_c{ci}" playOrder="{play_order}">
        <navLabel><text>{ch_name}</text></navLabel>
        <content src="Text/{vi}_{ci}.xhtml" />
      </navPoint>
"#,
                ch_name = escape_xml(&ch.title),
            ));
        }
        nav_map.push_str("    </navPoint>\n");
    }

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dtb:uid" content="wenku2epub" />
  </head>
  <docTitle><text>{title}</text></docTitle>
  <navMap>
{nav_map}  </navMap>
</ncx>
"#,
        title = escape_xml(&book.title),
    )
}
