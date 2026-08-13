use std::sync::Arc;

/// 下载过程中的进度事件，逻辑层只负责 emit，不关心前端如何消费
#[derive(Debug, Clone)]
pub enum Event {
    /// 开始处理书页
    StartBook,
    /// 开始解析目录
    StartParseToc,
    /// 目录解析完成
    ParseTocDone { volumes: usize, chapters: usize },
    /// 单章处理完成
    ChapterDone { current: usize, total: usize },
    /// 单张图片下载完成
    ImageDone { current: usize, total: usize },
    /// EPUB 生成完成，输出路径
    BookDone { path: String },
    /// 出错信息
    Error { message: String },
}

/// 事件回调：跨线程安全的闭包，由逻辑层持有并在关键节点调用
pub type EventEmitter = Arc<dyn Fn(Event) + Send + Sync>;

/// 消费事件的 trait，CLI 实现它来展示进度
pub trait DownloaderEvent {
    fn on_event(&self, event: Event);
}

/// 把 trait 实现者适配为 EventEmitter 闭包，供逻辑层调用
pub fn emitter_from<T: DownloaderEvent + Send + Sync + 'static>(ui: Arc<T>) -> EventEmitter {
    Arc::new(move |event| ui.on_event(event))
}
