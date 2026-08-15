mod color;

use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use downloader::book::EpubVersion;
use downloader::cover::CoverSource;
use downloader::error::{Error, ErrorKind, Result};
use downloader::manager::DownloadManager;
use downloader::model::{Selection, Stage};
use downloader::protocol::{Command, CommandOutcome, Event, EventSink, JobStatus};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use color::{
    cancel_mark, failure, failure_mark, menu_title, option, prompt, success, success_mark,
    title,
};

/// 读取一行输入
fn read_line(prompt_text: &str) -> Result<String> {
    print!("{}", prompt(prompt_text));
    io::stdout()
        .flush()
        .map_err(|e| Error::new(ErrorKind::Encode, format!("刷新输出失败: {e}")))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| Error::new(ErrorKind::Encode, format!("读取输入失败: {e}")))?;
    Ok(line.trim().to_string())
}

/// 选择封面类型
fn choose_cover_source() -> Result<CoverSource> {
    println!("{}", menu_title("请选择封面来源："));
    println!("  {} {}", option("1."), "轻小说文库封面");
    println!("  {} {}", option("2."), "第一卷的第一张图片");
    println!("  {} {}", option("3."), "当前目录的 cover.jpg/png 等图片");
    let choice = read_line("请输入序号 (1/2/3，默认 1)：")?;
    match choice.as_str() {
        "" | "1" => Ok(CoverSource::BookUrl),
        "2" => Ok(CoverSource::FirstImage),
        "3" => Ok(CoverSource::LocalFile),
        _ => Err(Error::new(ErrorKind::Parse, "无效的封面选择".into())),
    }
}

/// 选择 EPUB 版本
fn choose_version() -> Result<EpubVersion> {
    println!("{}", menu_title("请选择 EPUB 版本："));
    println!("  {} {}", option("1."), "EPUB 2 (toc.ncx)");
    println!("  {} {}", option("2."), "EPUB 3 (nav.xhtml)");
    let choice = read_line("请输入序号 (1/2，默认 2)：")?;
    match choice.as_str() {
        "" | "2" => Ok(EpubVersion::V3),
        "1" => Ok(EpubVersion::V2),
        _ => Err(Error::new(ErrorKind::Parse, "无效的版本选择".into())),
    }
}

/// 事件输出：只打印创建事件（任务结果由主循环统一打印，避免打断进度条）
struct CliSink;

impl EventSink for CliSink {
    fn emit(&self, event: Event) {
        if let Event::JobCreated { job_id, url } = event {
            println!(
                "{} 小说 #{job_id} 已创建：{url}",
                success_mark("◆")
            );
        }
    }
}

/// 根据阶段生成进度条消息
fn stage_message(job: &downloader::protocol::JobSnapshot) -> String {
    match job.stage {
        Stage::FetchBook => "抓取书页".to_string(),
        Stage::ParseToc => "解析目录".to_string(),
        Stage::DownloadChapters => {
            if job.chapters_total == 0 {
                "下载章节".to_string()
            } else {
                format!("下载章节 {}/{}", job.chapters_done, job.chapters_total)
            }
        }
        Stage::DownloadImages => {
            if job.images_total == 0 {
                "下载图片".to_string()
            } else {
                format!("下载图片 {}/{}", job.images_done, job.images_total)
            }
        }
        Stage::Pack => "打包 EPUB".to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", title("========== wenku2epub 小说下载器 =========="));
    let url = read_line("请输入要下载的小说网址：")?;
    if url.is_empty() {
        return Err(Error::new(ErrorKind::NotFound, "网址不能为空".into()));
    }
    let cover_source = choose_cover_source()?;
    let version = choose_version()?;

    let manager = DownloadManager::with_sink(1, 3, 5, cover_source, Arc::new(CliSink))?;

    // 创建并启动小说
    let job_id = match manager
        .dispatch(Command::CreateJob {
            url: url.clone(),
            selection: Selection::All,
            version,
        })
        .await?
    {
        CommandOutcome::Created(id) => id,
        _ => return Err(Error::new(ErrorKind::Encode, "创建小说失败".into())),
    };
    manager
        .dispatch(Command::StartJob { job_id })
        .await?;

    // 状态文本行 + 进度条行（分行显示，进度条不会因消息长度左右移动）
    let mp = MultiProgress::new();
    let status = mp.add(ProgressBar::new(1));
    status.set_style(ProgressStyle::with_template("{msg}").unwrap());
    status.set_message("准备中...");

    let pb = mp.add(ProgressBar::new(100));
    pb.set_style(
        ProgressStyle::with_template("{bar:40.blue} {pos}%")
            .unwrap()
            .progress_chars("█░"),
    );

    // 轮询快照直到结束
    loop {
        let snapshot = manager.get_snapshot();
        let mut all_done = true;
        for job in &snapshot {
            status.set_message(stage_message(job));
            pb.set_position(job.percent as u64);
            if !matches!(
                job.status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            ) {
                all_done = false;
            }
        }

        if all_done {
            status.finish(); // 保留状态行
            pb.finish(); // 保留进度条（停在 100%）
            if let Some(job) = snapshot.first() {
                match job.status {
                    JobStatus::Completed => {
                        println!(
                            "{} 小说 #{job_id} 完成：{}",
                            success_mark("✔"),
                            success(job.result_path.as_deref().unwrap_or("未知路径"))
                        );
                    }
                    JobStatus::Failed => {
                        println!(
                            "{} 小说 #{job_id} 失败：{}",
                            failure_mark("✘"),
                            failure(job.error.as_deref().unwrap_or("未知错误"))
                        );
                    }
                    JobStatus::Cancelled => {
                        println!("{} 小说 #{job_id} 已取消", cancel_mark("✘"));
                    }
                    _ => {}
                }
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}
