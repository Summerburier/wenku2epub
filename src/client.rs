use std::sync::atomic::{AtomicU64, Ordering};

use reqwest::{header, Client, StatusCode, Url};

use crate::error::{DownloadError, Result};
use crate::model::DownloaderConfig;

/// 轮询用的 UA 计数器，避免每次请求固定用同一个 UA
static UA_INDEX: AtomicU64 = AtomicU64::new(0);

/// 构建共享的 HTTP 客户端（复用连接池，避免每次请求重新握手）
pub fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .pool_max_idle_per_host(8)
        .build()
        .map_err(|e| DownloadError::Http(e.to_string()))
}

/// 默认 User-Agent 列表（与 JS 版对齐），可在配置中覆盖
pub fn default_user_agents() -> Vec<String> {
    vec![
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Edge/130.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".into(),
    ]
}

/// 轮询选取一个 UA
fn pick_user_agent(config: &DownloaderConfig) -> String {
    let list = if config.user_agents.is_empty() {
        default_user_agents()
    } else {
        config.user_agents.clone()
    };
    let idx = (UA_INDEX.fetch_add(1, Ordering::Relaxed) % list.len() as u64) as usize;
    list[idx].clone()
}

/// 从请求 URL 提取 origin 作为 Referer（如 https://www.wenku8.cc/）
fn referer_for(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.origin().ascii_serialization().parse().ok())
        .unwrap_or_else(|| {
            // 兜底：截取到第一个 '/' 之后的 scheme://host 部分
            url.split_once('/')
                .map(|(s, _)| s.to_string())
                .unwrap_or_default()
        })
        .to_string()
        + "/"
}

/// 若配置启用了延迟，在请求前等待 0.5~1 秒，避免规律性请求
async fn maybe_delay(config: &DownloaderConfig) {
    if config.delay_enabled {
        let base = 500;
        let random = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            % 500) as u64;
        tokio::time::sleep(std::time::Duration::from_millis(base + random)).await;
    }
}

/// 带重试的 HTTP GET，返回解码后的 UTF-8 文本
///
/// - 随机 UA + 动态 Referer（防反爬）
/// - 429 限流时读 Retry-After 头等待后重试，最多 max_retries 次
/// - 页面为 GBK 编码，统一解码为 UTF-8
pub async fn fetch(client: &Client, config: &DownloaderConfig, url: &str) -> Result<String> {
    maybe_delay(config).await;

    for attempt in 0..config.max_retries {
        let mut req = client.get(url).header(header::USER_AGENT, pick_user_agent(config));
        req = req
            .header(header::REFERER, referer_for(url))
            .header(header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6")
            .header(header::CONNECTION, "keep-alive")
            .header(header::DNT, "1")
            .header(header::UPGRADE_INSECURE_REQUESTS, "1");

        let resp = req.send().await.map_err(|e| DownloadError::Http(e.to_string()))?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5);
            if attempt + 1 < config.max_retries {
                tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
                continue;
            }
            return Err(DownloadError::Http(format!("请求 {url} 被限流（429）")));
        }

        if !resp.status().is_success() {
            return Err(DownloadError::Http(format!(
                "请求 {url} 返回状态码 {}",
                resp.status()
            )));
        }

        let bytes = resp.bytes().await.map_err(|e| DownloadError::Http(e.to_string()))?;
        return decode_gbk(&bytes, url);
    }

    Err(DownloadError::Http(format!("请求 {url} 重试 {} 次后仍失败", config.max_retries)))
}

/// 将页面字节按 GBK 解码为 UTF-8 字符串
fn decode_gbk(bytes: &[u8], url: &str) -> Result<String> {
    let (cow, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if had_errors {
        // 解码有错误，尝试宽松模式兜底
        let (cow, _, _) = encoding_rs::GBK.decode(bytes);
        return Ok(cow.into_owned());
    }
    let _ = url;
    Ok(cow.into_owned())
}

/// 下载图片等二进制资源，返回原始字节
pub async fn download_bytes(client: &Client, config: &DownloaderConfig, src: &str) -> Result<Vec<u8>> {
    maybe_delay(config).await;

    for attempt in 0..config.max_retries {
        let resp = client
            .get(src)
            .header(header::USER_AGENT, pick_user_agent(config))
            .header(header::REFERER, referer_for(src))
            .header(header::ACCEPT, "image/webp,image/apng,image/*,*/*;q=0.8")
            .send()
            .await
            .map_err(|e| DownloadError::Http(e.to_string()))?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5);
            if attempt + 1 < config.max_retries {
                tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
                continue;
            }
            return Err(DownloadError::Http(format!("图片 {src} 被限流（429）")));
        }

        if !resp.status().is_success() {
            return Err(DownloadError::Http(format!(
                "图片 {src} 返回状态码 {}",
                resp.status()
            )));
        }

        return resp
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| DownloadError::Http(e.to_string()));
    }

    Err(DownloadError::Http(format!("图片 {src} 重试 {} 次后仍失败", config.max_retries)))
}
