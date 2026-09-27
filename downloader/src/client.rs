use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{header, Client, StatusCode, Url};
use tokio::sync::Notify;

use crate::error::{Error, ErrorKind, Result};

static UA_INDEX: AtomicU64 = AtomicU64::new(0);
static RETRY_JITTER: AtomicU64 = AtomicU64::new(0);

/// 最大尝试次数
const MAX_ATTEMPTS: u32 = 10;

/// 重试间隔（毫秒）
const RETRY_DELAYS_MS: [u64; 6] = [50, 100, 150, 200, 300, 600];

/// 按尝试次数返回等待延迟：首次不等待，重试依次取 [50,100,150,200,300,600]
async fn retry_delay(attempt: u32) {
    if attempt == 0 {
        return;
    }
    let idx = ((attempt - 1) as usize).min(RETRY_DELAYS_MS.len() - 1);
    tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAYS_MS[idx])).await;
}

/// 429 表示服务器正在限流：优先遵守 Retry-After，否则指数退避。
async fn rate_limit_delay(resp: &reqwest::Response, attempt: u32) {
    let retry_after = resp
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| 1u64 << attempt.min(3));
    // 让同时被限流的任务错开重试，避免在同一时刻再次冲击服务器。
    let jitter_ms = RETRY_JITTER.fetch_add(73, Ordering::Relaxed) % 251;
    tokio::time::sleep(Duration::from_secs(retry_after) + Duration::from_millis(jitter_ms)).await;
}

#[derive(Debug)]
struct AdaptiveState {
    limit: usize,
    in_flight: usize,
    successes: usize,
    last_decrease: Option<Instant>,
}

/// 所有分卷共享的自适应并发控制器。
///
/// 从最大并发开始；收到 429 时将并发减半，连续成功后逐步恢复。
#[derive(Debug)]
pub struct AdaptiveLimiter {
    state: Mutex<AdaptiveState>,
    notify: Notify,
    min_limit: usize,
    max_limit: usize,
}

impl AdaptiveLimiter {
    pub fn new(max_limit: usize) -> Arc<Self> {
        let max_limit = max_limit.max(1);
        Arc::new(Self {
            state: Mutex::new(AdaptiveState {
                limit: max_limit,
                in_flight: 0,
                successes: 0,
                last_decrease: None,
            }),
            notify: Notify::new(),
            min_limit: 1,
            max_limit,
        })
    }

    pub async fn acquire(self: &Arc<Self>) -> AdaptivePermit {
        loop {
            // 先创建通知 future，避免在检查状态与等待之间丢失唤醒。
            let notified = self.notify.notified();
            {
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if state.in_flight < state.limit {
                    state.in_flight += 1;
                    return AdaptivePermit {
                        limiter: self.clone(),
                    };
                }
            }
            notified.await;
        }
    }

    pub fn record_success(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.successes += 1;
        // 至少经历当前并发量的两轮成功才加 1，避免恢复过快。
        if state.limit < self.max_limit && state.successes >= state.limit * 2 {
            state.limit += 1;
            state.successes = 0;
            self.notify.notify_waiters();
        }
    }

    pub fn record_rate_limit(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.successes = 0;
        let now = Instant::now();
        // 同一批并发请求可能同时返回 429，1 秒内只降一次，防止瞬间跌到 1。
        let can_decrease = state
            .last_decrease
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1));
        if can_decrease {
            state.limit = (state.limit / 2).max(self.min_limit);
            state.last_decrease = Some(now);
        }
    }

    pub fn current_limit(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .limit
    }
}

pub struct AdaptivePermit {
    limiter: Arc<AdaptiveLimiter>,
}

impl Drop for AdaptivePermit {
    fn drop(&mut self) {
        let mut state = self
            .limiter
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        state.in_flight = state.in_flight.saturating_sub(1);
        drop(state);
        self.limiter.notify.notify_one();
    }
}

/// 默认 User-Agent 列表
pub fn default_user_agents() -> Vec<String> {
    vec![
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0".into(),
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15".into(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Edge/130.0.0.0 Safari/537.36".into(),
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1".into(),
    ]
}

fn pick_user_agent() -> String {
    let list = default_user_agents();
    let idx = (UA_INDEX.fetch_add(1, Ordering::Relaxed) % list.len() as u64) as usize;
    list[idx].clone()
}

fn referer_for(url: &str) -> String {
    Url::parse(url)
        .ok()
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_default()
        .to_string()
        + "/"
}

/// 浏览器风格的 HTML 页面请求头
fn html_headers(url: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::USER_AGENT, HeaderValue::from_str(&pick_user_agent()).expect("解析章节错误"));
    h.insert(header::REFERER, HeaderValue::from_str(&referer_for(url)).expect("解析章节错误"));
    h.insert(
        header::ACCEPT_LANGUAGE,
        HeaderValue::from_str("zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6").expect("解析章节错误"),
    );
    h.insert(
        header::ACCEPT,
        HeaderValue::from_str("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8").expect("解析章节错误"),
    );
    h.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    h.insert("DNT", HeaderValue::from_static("1"));
    h.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
    h.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
    h.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
    h.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    h.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));
    h
}

/// 浏览器风格的图片请求头
fn image_headers(src: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::USER_AGENT, HeaderValue::from_str(&pick_user_agent()).expect("解析章节错误"));
    h.insert(header::REFERER, HeaderValue::from_str(&referer_for(src)).expect("解析章节错误"));
    h.insert(
        header::ACCEPT,
        HeaderValue::from_static("image/webp,image/apng,image/*,*/*;q=0.8"),
    );
    h
}

/// 提取 URL 的 origin，用作图片防盗链 Referer
pub fn origin_for(url: &str) -> String {
    Url::parse(url)
        .ok()
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_default()
        .to_string()
        + "/"
}

/// 构建共享 HTTP 客户端
pub fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .pool_max_idle_per_host(16)
        .build()
        .map_err(|e| Error::new(ErrorKind::Network, format!("构建 HTTP 客户端失败: {e}")))
}

/// 带重试的 HTTP GET，获取 HTML 页面，返回解码后的 UTF-8 文本（GBK 解码）
/// 所有失败统一重试，循环耗尽后统一返回"url:尝试次数:错误代码"
pub async fn fetch_html(client: &Client, url: &str) -> Result<String> {
    fetch_html_inner(client, url, None).await
}

pub async fn fetch_html_limited(
    client: &Client,
    url: &str,
    limiter: &Arc<AdaptiveLimiter>,
) -> Result<String> {
    fetch_html_inner(client, url, Some(limiter)).await
}

async fn fetch_html_inner(
    client: &Client,
    url: &str,
    limiter: Option<&Arc<AdaptiveLimiter>>,
) -> Result<String> {
    let mut last_error = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        // 首次（attempt == 0）不等待；失败后的重试前按数组延迟
        retry_delay(attempt).await;
        let adaptive_permit = match limiter {
            Some(limiter) => Some(limiter.acquire().await),
            None => None,
        };

        let resp = match client
            .get(url)
            .headers(html_headers(url))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = e.to_string(); // 网络错误
                drop(adaptive_permit);
                continue;
            }
        };

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            if let Some(limiter) = limiter {
                limiter.record_rate_limit();
            }
            drop(adaptive_permit);
            rate_limit_delay(&resp, attempt).await;
            last_error = "429".to_string(); // 被限流
            continue;
        }

        if !resp.status().is_success() {
            // 5xx/408 是临时错误，重试；其他 4xx（403/404/400...）是永久错误，直接抛出
            if resp.status().is_server_error() || resp.status() == StatusCode::REQUEST_TIMEOUT {
                last_error = resp.status().as_str().to_string(); // 状态码，如 "503"
                drop(adaptive_permit);
                continue;
            }
            drop(adaptive_permit);
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("{url},{},{}", attempt + 1, resp.status().as_str()),
            ));
        }

        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                last_error = e.to_string(); // 读取失败
                drop(adaptive_permit);
                continue;
            }
        };
        if let Some(limiter) = limiter {
            limiter.record_success();
        }
        drop(adaptive_permit);
        let (cow, _, _) = encoding_rs::GBK.decode(&bytes);
        return Ok(cow.into_owned());
    }

    Err(Error::new(
        ErrorKind::Network,
        format!("{url},{MAX_ATTEMPTS},{last_error}"),
    ))
}

/// 带重试的 HTTP GET，获取二进制资源（图片），返回原始字节。
/// referer 应指向来源站点（wenku8 域名），图片服务器按此防盗链放行。
/// 普通失败延迟 50ms 重试，429 则遵守 Retry-After 或指数退避。
/// 循环耗尽后统一返回"url:尝试次数:错误代码"
pub async fn fetch_bytes(client: &Client, src: &str) -> Result<Vec<u8>> {
    fetch_bytes_inner(client, src, None).await
}

pub async fn fetch_bytes_limited(
    client: &Client,
    src: &str,
    limiter: &Arc<AdaptiveLimiter>,
) -> Result<Vec<u8>> {
    fetch_bytes_inner(client, src, Some(limiter)).await
}

async fn fetch_bytes_inner(
    client: &Client,
    src: &str,
    limiter: Option<&Arc<AdaptiveLimiter>>,
) -> Result<Vec<u8>> {
    let mut last_error = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        // 首次（attempt == 0）不等待；失败后的重试前固定等 50ms
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let adaptive_permit = match limiter {
            Some(limiter) => Some(limiter.acquire().await),
            None => None,
        };

        let resp = match client
            .get(src)
            .headers(image_headers(src))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = e.to_string(); // 网络错误
                drop(adaptive_permit);
                continue;
            }
        };

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            if let Some(limiter) = limiter {
                limiter.record_rate_limit();
            }
            drop(adaptive_permit);
            rate_limit_delay(&resp, attempt).await;
            last_error = "429".to_string();
            continue;
        }

        if !resp.status().is_success() {
            last_error = resp.status().as_str().to_string(); // 状态码，如 "403"
            drop(adaptive_permit);
            continue;
        }

        let bytes = match resp.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                last_error = e.to_string(); // 读取失败
                drop(adaptive_permit);
                continue;
            }
        };
        if let Some(limiter) = limiter {
            limiter.record_success();
        }
        drop(adaptive_permit);
        return Ok(bytes);
    }

    Err(Error::new(
        ErrorKind::Network,
        format!("{src},{MAX_ATTEMPTS},{last_error}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::AdaptiveLimiter;

    #[test]
    fn adaptive_limiter_halves_then_recovers_slowly() {
        let limiter = AdaptiveLimiter::new(12);

        limiter.record_rate_limit();
        assert_eq!(limiter.current_limit(), 6);

        // 同一批 429 不应连续将并发压到 1。
        limiter.record_rate_limit();
        assert_eq!(limiter.current_limit(), 6);

        // 当前限制的两轮请求都成功后，只恢复 1 个并发。
        for _ in 0..12 {
            limiter.record_success();
        }
        assert_eq!(limiter.current_limit(), 7);
    }
}
