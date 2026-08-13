/// 统一错误类型，供逻辑层各模块使用，不依赖任何绑定层类型
#[derive(Debug)]
pub enum DownloadError {
    /// 网络请求失败
    Http(String),
    /// 页面结构解析失败（可能网站改版）
    Parse(String),
    /// 编码转换失败
    Encode(String),
    /// 未找到目录/章节
    NotFound(String),
    /// 用户取消
    Cancelled,
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Http(m) => write!(f, "HTTP 错误: {m}"),
            DownloadError::Parse(m) => write!(f, "解析错误: {m}"),
            DownloadError::Encode(m) => write!(f, "编码错误: {m}"),
            DownloadError::NotFound(m) => write!(f, "未找到: {m}"),
            DownloadError::Cancelled => write!(f, "已取消"),
        }
    }
}

impl std::error::Error for DownloadError {}

pub type Result<T> = std::result::Result<T, DownloadError>;
