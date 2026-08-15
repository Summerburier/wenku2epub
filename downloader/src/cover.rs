use std::collections::HashMap;
use std::path::Path;

use reqwest::Client;

use crate::client::fetch_bytes;
use crate::error::Result;
use crate::model::Book;

/// 封面获取策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSource {
    /// 1. 读取 book.cover_url 下载图片作为封面
    BookUrl,
    /// 2. 读取第一卷的第一张图片作为封面
    FirstImage,
    /// 3. 读取当前路径下的 cover.jpg/png 等图片作为封面
    LocalFile,
}

/// 本地封面候选文件名（按优先级排列）
const LOCAL_COVER_NAMES: [&str; 6] = [
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "cover.gif",
    "cover.webp",
    "cover.bmp",
];

/// 按策略获取封面字节；无法获取时返回 None（不视为错误）
pub async fn resolve_cover(
    client: &Client,
    book: &Book,
    downloaded: &HashMap<String, Vec<u8>>,
    source: CoverSource,
) -> Result<Option<Vec<u8>>> {
    match source {
        CoverSource::BookUrl => match &book.cover_url {
            Some(url) => match fetch_bytes(client, url).await {
                Ok(bytes) => Ok(Some(bytes)),
                Err(e) => {
                    eprintln!("封面 URL 下载失败（跳过封面）：{e}");
                    Ok(None)
                }
            },
            None => {
                eprintln!("未解析到封面 URL，跳过封面");
                Ok(None)
            }
        },

        CoverSource::FirstImage => {
            // 第一卷的图片文件名为 "0_<章>_<图>.jpg"，
            // 按 (卷, 章, 图) 数值排序（而非字符串字典序，避免 0_0_10 < 0_0_2 的陷阱），取第一张
            let mut keys: Vec<&String> = downloaded
                .keys()
                .filter(|k| k.starts_with("0_"))
                .collect();
            keys.sort_by_key(|k| {
                let parts: Vec<usize> = k
                    .trim_end_matches(".jpg")
                    .split('_')
                    .filter_map(|p| p.parse::<usize>().ok())
                    .collect();
                (
                    parts.first().copied().unwrap_or(0),
                    parts.get(1).copied().unwrap_or(0),
                    parts.get(2).copied().unwrap_or(0),
                )
            });
            match keys.first() {
                Some(key) => Ok(downloaded.get(*key).cloned()),
                None => {
                    eprintln!("第一卷没有图片，跳过封面");
                    Ok(None)
                }
            }
        }

        CoverSource::LocalFile => {
            for name in LOCAL_COVER_NAMES {
                if Path::new(name).is_file() {
                    match std::fs::read(name) {
                        Ok(bytes) => return Ok(Some(bytes)),
                        Err(e) => {
                            eprintln!("读取本地封面 {name} 失败：{e}");
                        }
                    }
                }
            }
            eprintln!("当前目录未找到本地封面文件（cover.jpg/png 等），跳过封面");
            Ok(None)
        }
    }
}
