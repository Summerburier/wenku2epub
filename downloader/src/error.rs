#[derive(Debug)]
pub struct Error {
    /// 错误类型
    pub kind: ErrorKind,
    /// 错误信息
    pub message: String,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorKind {
    /// 网络错误
    Network,
    /// 解析错误
    Parse,
    /// 编码错误
    Encode,
    /// 未找到
    NotFound,
    /// 已取消
    Cancelled,
}


impl Error {
    pub fn new(kind: ErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::Network => write!(f, "网络错误: {}", self.message),
            ErrorKind::Parse => write!(f, "解析错误: {}", self.message),
            ErrorKind::Encode => write!(f, "编码错误: {}", self.message),
            ErrorKind::NotFound => write!(f, "未找到: {}", self.message),
            ErrorKind::Cancelled => write!(f, "已取消: {}", self.message),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;