mod color;

use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use console::{Key, Term, style};
use downloader::book::EpubVersion;
use downloader::cover::CoverSource;
use downloader::error::{Error, ErrorKind, Result};
use downloader::manager::DownloadManager;
use downloader::model::{Selection, Stage};
use downloader::protocol::{Command, CommandOutcome, Event, EventSink, JobStatus};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use color::{
    cancel_mark, failure, failure_mark, menu_title, option, prompt, success, success_mark, title,
};

/// 使用方向键移动、空格选择、Enter 确认的终端单选菜单。
fn select_option(title_text: &str, choices: &[String], default: usize) -> Result<usize> {
    debug_assert!(!choices.is_empty());
    debug_assert!(default < choices.len());

    let term = Term::stdout();
    let mut cursor = default;
    let mut selected = default;
    let line_count = choices.len() + 1;

    term.hide_cursor()
        .map_err(|e| Error::new(ErrorKind::Encode, format!("隐藏光标失败: {e}")))?;

    let selection_result = (|| -> Result<usize> {
        loop {
            term.write_line(&menu_title(title_text))
                .map_err(|e| Error::new(ErrorKind::Encode, format!("写入菜单失败: {e}")))?;

            for (index, label) in choices.iter().enumerate() {
                let pointer = if index == cursor {
                    option("❯")
                } else {
                    " ".into()
                };
                let marker = if index == selected {
                    success_mark("●")
                } else {
                    "○".into()
                };
                let label = if index == cursor {
                    style(label).bold().to_string()
                } else {
                    label.clone()
                };
                term.write_line(&format!("  {pointer} {marker} {label}"))
                    .map_err(|e| Error::new(ErrorKind::Encode, format!("写入选项失败: {e}")))?;
            }

            match term
                .read_key()
                .map_err(|e| Error::new(ErrorKind::Encode, format!("读取按键失败: {e}")))?
            {
                Key::ArrowUp => cursor = (cursor + choices.len() - 1) % choices.len(),
                Key::ArrowDown => cursor = (cursor + 1) % choices.len(),
                Key::Char(' ') => selected = cursor,
                Key::Enter => break Ok(selected),
                Key::Escape => {
                    break Err(Error::new(ErrorKind::Cancelled, "用户取消了选择".into()));
                }
                _ => {}
            }

            term.clear_last_lines(line_count)
                .map_err(|e| Error::new(ErrorKind::Encode, format!("刷新菜单失败: {e}")))?;
        }
    })();

    let show_cursor_result = term.show_cursor();
    match selection_result {
        Ok(index) => {
            term.clear_last_lines(line_count)
                .map_err(|e| Error::new(ErrorKind::Encode, format!("收起菜单失败: {e}")))?;
            term.write_line(&format!(
                "{} {} {}",
                success_mark("◆"),
                title_text.trim_end_matches('：'),
                success(&choices[index])
            ))
            .map_err(|e| Error::new(ErrorKind::Encode, format!("写入选择结果失败: {e}")))?;
            show_cursor_result
                .map_err(|e| Error::new(ErrorKind::Encode, format!("恢复光标失败: {e}")))?;
            Ok(index)
        }
        Err(error) => {
            let _ = show_cursor_result;
            Err(error)
        }
    }
}

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
    let choices = [
        "轻小说文库封面",
        "第一卷的第一张图片",
        "当前目录的 cover.jpg/png 等图片",
    ]
    .map(str::to_owned);
    match select_option("请选择封面来源：", &choices, 0)? {
        0 => Ok(CoverSource::BookUrl),
        1 => Ok(CoverSource::FirstImage),
        _ => Ok(CoverSource::LocalFile),
    }
}

/// 选择 EPUB 版本
fn choose_version() -> Result<EpubVersion> {
    let choices = ["EPUB 2 (toc.ncx)", "EPUB 3 (nav.xhtml)"].map(str::to_owned);
    match select_option("请选择 EPUB 版本：", &choices, 1)? {
        0 => Ok(EpubVersion::V2),
        _ => Ok(EpubVersion::V3),
    }
}

/// 展示解析出的书名，选择书名格式（完整 / 括号内 / 括号前）
fn choose_title_style(
    parts: &downloader::parser::TitleParts,
) -> Result<downloader::model::TitleStyle> {
    println!("{} 解析到书名：{}", success_mark("◆"), success(&parts.full));
    let choices = [
        format!("完整书名：{}", parts.full),
        parts
            .in_bracket
            .as_ref()
            .map(|value| format!("括号中的书名：{value}"))
            .unwrap_or_else(|| "括号中的书名（无括号）".into()),
        parts
            .before_bracket
            .as_ref()
            .map(|value| format!("括号前的书名：{value}"))
            .unwrap_or_else(|| "括号前的书名（无括号）".into()),
    ];
    match select_option("请选择书名格式：", &choices, 0)? {
        0 => Ok(downloader::model::TitleStyle::Full),
        1 => Ok(downloader::model::TitleStyle::InBracket),
        _ => Ok(downloader::model::TitleStyle::BeforeBracket),
    }
}

/// 事件输出：只打印创建事件（任务结果由主循环统一打印，避免打断进度条）
struct CliSink;

impl EventSink for CliSink {
    fn emit(&self, event: Event) {
        if let Event::JobCreated { job_id, url } = event {
            println!("{} 小说 #{job_id} 已创建：{url}", success_mark("◆"));
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
    println!("{}", prompt("提示：使用 ↑/↓ 移动，空格选择，Enter 确认"));
    let url = read_line("请输入要下载的小说网址：")?;
    if url.is_empty() {
        return Err(Error::new(ErrorKind::NotFound, "网址不能为空".into()));
    }

    // 预解析书页，展示书名供用户选择格式
    let client = downloader::client::build_client()?;
    let html = downloader::client::fetch_html(&client, &url).await?;
    let mut book = downloader::model::Book::default();
    downloader::parser::parse_book_info(&html, &url, &mut book)?;
    let title_parts = downloader::parser::parse_title(&book.title);
    let title_style = choose_title_style(&title_parts)?;

    let cover_source = choose_cover_source()?;
    let version = choose_version()?;

    let manager = DownloadManager::with_sink(1, 3, 5, cover_source, Arc::new(CliSink))?;

    // 创建并启动小说
    let job_id = match manager
        .dispatch(Command::CreateJob {
            url: url.clone(),
            selection: Selection::All,
            version,
            title_style,
        })
        .await?
    {
        CommandOutcome::Created(id) => id,
        _ => return Err(Error::new(ErrorKind::Encode, "创建小说失败".into())),
    };
    manager.dispatch(Command::StartJob { job_id }).await?;

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
