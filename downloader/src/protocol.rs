use crate::book::EpubVersion;
use crate::model::{Selection, Stage};

/// 任务 ID
pub type JobId = u64;

/// 前端 → 后端：命令
#[derive(Debug, Clone)]
pub enum Command {
    /// 创建任务（加入队列）
    CreateJob {
        url: String,
        selection: Selection,
        version: EpubVersion,
    },
    /// 开始执行任务
    StartJob { job_id: JobId },
    /// 取消任务
    CancelJob { job_id: JobId },
    /// 取消所有任务
    CancelAll,
    /// 清理已结束的任务
    RemoveJob { job_id: JobId },
    /// 拉取所有任务快照
    GetSnapshot,
}

/// 命令执行结果
#[derive(Debug, Clone)]
pub enum CommandOutcome {
    /// 无特殊结果
    None,
    /// CreateJob 返回新任务 ID
    Created(JobId),
    /// GetSnapshot 返回快照
    Snapshot(Vec<JobSnapshot>),
}

/// 后端 → 前端：事件（低频生命周期信号）
#[derive(Debug, Clone)]
pub enum Event {
    JobCreated { job_id: JobId, url: String },
    JobCompleted { job_id: JobId, path: String },
    JobFailed { job_id: JobId, message: String },
    JobCancelled { job_id: JobId },
}

/// 任务状态机
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// 前端可读的任务快照（DTO）
#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub job_id: JobId,
    pub url: String,
    pub status: JobStatus,
    pub stage: Stage,
    /// 全局进度 0~100
    pub percent: u32,
    pub chapters_done: usize,
    pub chapters_total: usize,
    pub images_done: usize,
    pub images_total: usize,
    pub result_path: Option<String>,
    pub error: Option<String>,
}

/// 事件接收器（CLI 打印 / Tauri emit）
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: Event);
}
