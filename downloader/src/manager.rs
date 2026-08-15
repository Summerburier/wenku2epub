use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use reqwest::Client;
use tokio::sync::Semaphore;

use crate::book::{self, EpubVersion};
use crate::client::build_client;
use crate::cover::CoverSource;
use crate::error::{ErrorKind, Result};
use crate::model::{Progress, Selection, Stage};
use crate::protocol::{
    Command, CommandOutcome, Event, EventSink, JobId, JobSnapshot, JobStatus,
};

/// 一个下载任务
pub struct Job {
    pub id: JobId,
    pub url: String,
    pub selection: Selection,
    pub version: EpubVersion,
    pub status: JobStatus,
    pub progress: Arc<Progress>,
    pub result_path: Option<String>,
    pub error: Option<String>,
}

/// 下载管理器：多任务池化调度
pub struct DownloadManager {
    jobs: Arc<Mutex<HashMap<JobId, Job>>>,
    next_id: AtomicU64,
    /// 同时最多运行几个任务
    pool: Arc<Semaphore>,
    /// 共享 HTTP 客户端
    client: Client,
    /// 章节下载并发数
    concurrency: usize,
    /// 图片下载并发数
    image_concurrency: usize,
    /// 封面获取策略
    cover_source: CoverSource,
    sink: Option<Arc<dyn EventSink>>,
}

impl DownloadManager {
    pub fn new(
        max_jobs: usize,
        concurrency: usize,
        image_concurrency: usize,
        cover_source: CoverSource,
    ) -> Result<Self> {
        let client = build_client()?;
        Ok(Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(0),
            pool: Arc::new(Semaphore::new(max_jobs)),
            client,
            concurrency,
            image_concurrency,
            cover_source,
            sink: None,
        })
    }

    pub fn with_sink(
        max_jobs: usize,
        concurrency: usize,
        image_concurrency: usize,
        cover_source: CoverSource,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self> {
        let mut manager = Self::new(max_jobs, concurrency, image_concurrency, cover_source)?;
        manager.sink = Some(sink);
        Ok(manager)
    }

    fn emit(&self, event: Event) {
        if let Some(sink) = &self.sink {
            sink.emit(event);
        }
    }

    /// 统一命令入口
    pub async fn dispatch(&self, cmd: Command) -> Result<CommandOutcome> {
        match cmd {
            Command::CreateJob {
                url,
                selection,
                version,
            } => {
                let job_id = self.create_job(url, selection, version);
                Ok(CommandOutcome::Created(job_id))
            }
            Command::StartJob { job_id } => {
                self.start_job(job_id).await;
                Ok(CommandOutcome::None)
            }
            Command::CancelJob { job_id } => {
                self.cancel_job(job_id);
                Ok(CommandOutcome::None)
            }
            Command::CancelAll => {
                self.cancel_all();
                Ok(CommandOutcome::None)
            }
            Command::RemoveJob { job_id } => {
                self.remove_job(job_id);
                Ok(CommandOutcome::None)
            }
            Command::GetSnapshot => Ok(CommandOutcome::Snapshot(self.get_snapshot())),
        }
    }

    fn create_job(&self, url: String, selection: Selection, version: EpubVersion) -> JobId {
        let job_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let job = Job {
            id: job_id,
            url: url.clone(),
            selection,
            version,
            status: JobStatus::Queued,
            progress: Arc::new(Progress::default()),
            result_path: None,
            error: None,
        };
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job_id, job);
        self.emit(Event::JobCreated {
            job_id,
            url: url.clone(),
        });
        job_id
    }

    async fn start_job(&self, job_id: JobId) {
        let (url, selection, version, progress) = {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            let job = match jobs.get_mut(&job_id) {
                Some(j) => j,
                None => return,
            };
            if job.status != JobStatus::Queued {
                return;
            }
            job.status = JobStatus::Running;
            (
                job.url.clone(),
                job.selection.clone(),
                job.version,
                job.progress.clone(),
            )
        };

        let pool = self.pool.clone();
        let jobs_map = self.jobs.clone();
        let client = self.client.clone();
        let sink = self.sink.clone();
        let concurrency = self.concurrency;
        let image_concurrency = self.image_concurrency;
        let cover_source = self.cover_source;

        tokio::spawn(async move {
            let _permit = match pool.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };

            let result = book::generate_book(
                &client,
                &url,
                &selection,
                concurrency,
                image_concurrency,
                version,
                cover_source,
                &progress,
            )
            .await;

            let mut jobs = jobs_map.lock().unwrap_or_else(|e| e.into_inner());
            let job = match jobs.get_mut(&job_id) {
                Some(j) => j,
                None => return,
            };

            match result {
                Ok(result) => {
                    job.status = JobStatus::Completed;
                    job.result_path = Some(result.path.clone());
                    if let Some(s) = &sink {
                        s.emit(Event::JobCompleted {
                            job_id,
                            path: result.path.clone(),
                        });
                    }
                }
                Err(e) => {
                    if e.kind == ErrorKind::Cancelled {
                        job.status = JobStatus::Cancelled;
                        if let Some(s) = &sink {
                            s.emit(Event::JobCancelled { job_id });
                        }
                    } else {
                        job.status = JobStatus::Failed;
                        job.error = Some(e.message.clone());
                        if let Some(s) = &sink {
                            s.emit(Event::JobFailed {
                                job_id,
                                message: e.message.clone(),
                            });
                        }
                    }
                }
            }
        });
    }

    fn cancel_job(&self, job_id: JobId) {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.get(&job_id) {
            job.progress.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn cancel_all(&self) {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        for job in jobs.values() {
            job.progress.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn remove_job(&self, job_id: JobId) {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let finished = matches!(
            jobs.get(&job_id).map(|j| j.status),
            Some(JobStatus::Completed) | Some(JobStatus::Failed) | Some(JobStatus::Cancelled)
        );
        if finished {
            jobs.remove(&job_id);
        }
    }

    /// 计算全局进度百分比（按阶段权重折算）
    fn calc_percent(progress: &Progress) -> u32 {
        let chapters_total = progress.chapters_total.load(Ordering::Relaxed);
        let chapters_done = progress.chapters_done.load(Ordering::Relaxed);
        let images_total = progress.images_total.load(Ordering::Relaxed);
        let images_done = progress.images_done.load(Ordering::Relaxed);

        match progress.get_stage() {
            Stage::FetchBook => 2,
            Stage::ParseToc => 5,
            Stage::DownloadChapters => {
                if chapters_total == 0 {
                    10
                } else {
                    10 + (30 * chapters_done / chapters_total) as u32
                }
            }
            Stage::DownloadImages => {
                if images_total == 0 {
                    40
                } else {
                    40 + (60 * images_done / images_total) as u32
                }
            }
            Stage::Pack => 100,
        }
    }

    /// 拉取所有任务快照
    pub fn get_snapshot(&self) -> Vec<JobSnapshot> {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.values()
            .map(|job| JobSnapshot {
                job_id: job.id,
                url: job.url.clone(),
                status: job.status,
                stage: job.progress.get_stage(),
                percent: Self::calc_percent(&job.progress),
                chapters_done: job.progress.chapters_done.load(Ordering::Relaxed),
                chapters_total: job.progress.chapters_total.load(Ordering::Relaxed),
                images_done: job.progress.images_done.load(Ordering::Relaxed),
                images_total: job.progress.images_total.load(Ordering::Relaxed),
                result_path: job.result_path.clone(),
                error: job.error.clone(),
            })
            .collect()
    }
}
