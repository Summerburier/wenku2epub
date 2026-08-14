#[derive(Debug)]
pub struct Error {
    /// 错误类型
    pub kind: ErrorKind,
    /// 错误信息
    pub message: String,
}
#[derive(Debug)]
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
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}