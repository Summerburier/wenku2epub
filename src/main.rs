mod color;

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use console::{Key, Term, style, truncate_str};
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

/// 使用方框多选：方向键移动、空格切换、Enter 确认。
fn select_multiple(title_text: &str, choices: &[String]) -> Result<Vec<usize>> {
    debug_assert!(!choices.is_empty());

    let term = Term::stdout();
    let mut cursor = 0usize;
    let mut window_start = 0usize;
    let mut selected = vec![false; choices.len()];
    let (terminal_rows, terminal_columns) = term.size();
    // 为标题、页码和操作提示留出空间，避免整份卷列表将视口推到最底部。
    let visible_count = choices
        .len()
        .min(usize::from(terminal_rows).saturating_sub(5).max(1));
    let line_count = visible_count + 3;
    // 前缀“  ❯ ☐ ”占 6 列，再留 1 列防止终端在右边界自动换行。
    let label_width = usize::from(terminal_columns).saturating_sub(7).max(1);

    term.hide_cursor()
        .map_err(|e| Error::new(ErrorKind::Encode, format!("隐藏光标失败: {e}")))?;

    let selection_result = (|| -> Result<Vec<usize>> {
        loop {
            if cursor < window_start {
                window_start = cursor;
            } else if cursor >= window_start + visible_count {
                window_start = cursor + 1 - visible_count;
            }
            let window_end = (window_start + visible_count).min(choices.len());

            term.write_line(&menu_title(title_text))
                .map_err(|e| Error::new(ErrorKind::Encode, format!("写入菜单失败: {e}")))?;
            term.write_line(&format!(
                "  显示 {}-{} / 共 {} 卷",
                window_start + 1,
                window_end,
                choices.len()
            ))
            .map_err(|e| Error::new(ErrorKind::Encode, format!("写入分页失败: {e}")))?;
            for (index, label) in choices
                .iter()
                .enumerate()
                .take(window_end)
                .skip(window_start)
            {
                let pointer = if index == cursor {
                    option("❯")
                } else {
                    " ".into()
                };
                let marker = if selected[index] {
                    success_mark("☑")
                } else {
                    "☐".into()
                };
                let label = truncate_str(label, label_width, "…").into_owned();
                let label = if index == cursor {
                    style(&label).bold().to_string()
                } else {
                    label
                };
                term.write_line(&format!("  {pointer} {marker} {label}"))
                    .map_err(|e| Error::new(ErrorKind::Encode, format!("写入选项失败: {e}")))?;
            }
            let hint = if selected.iter().any(|value| *value) {
                "空格切换，Enter 完成"
            } else {
                "请至少选择一卷"
            };
            term.write_line(&format!("  {}", prompt(hint)))
                .map_err(|e| Error::new(ErrorKind::Encode, format!("写入提示失败: {e}")))?;

            match term
                .read_key()
                .map_err(|e| Error::new(ErrorKind::Encode, format!("读取按键失败: {e}")))?
            {
                Key::ArrowUp => cursor = cursor.saturating_sub(1),
                Key::ArrowDown => cursor = (cursor + 1).min(choices.len() - 1),
                Key::Char(' ') => selected[cursor] = !selected[cursor],
                Key::Enter if selected.iter().any(|value| *value) => {
                    break Ok(selected
                        .iter()
                        .enumerate()
                        .filter_map(|(index, value)| value.then_some(index))
                        .collect());
                }
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
        Ok(indices) => {
            term.clear_last_lines(line_count)
                .map_err(|e| Error::new(ErrorKind::Encode, format!("收起菜单失败: {e}")))?;
            let labels = indices
                .iter()
                .map(|index| choices[*index].as_str())
                .collect::<Vec<_>>()
                .join("、");
            term.write_line(&format!(
                "{} {} {}",
                success_mark("◆"),
                title_text.trim_end_matches('：'),
                success(&labels)
            ))
            .map_err(|e| Error::new(ErrorKind::Encode, format!("写入选择结果失败: {e}")))?;
            show_cursor_result
                .map_err(|e| Error::new(ErrorKind::Encode, format!("恢复光标失败: {e}")))?;
            Ok(indices)
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

/// 全卷模式下选择封面类型。
fn choose_cover_source() -> Result<CoverSource> {
    let choices = [
        "轻小说文库封面".to_owned(),
        "第一卷的第一张图片".to_owned(),
        "当前目录的 cover.jpg/png 等图片".to_owned(),
    ];
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

    let toc_url = book
        .toc_url
        .clone()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "未找到目录链接".into()))?;
    let toc_html = downloader::client::fetch_html(&client, &toc_url).await?;
    downloader::parser::parse_toc(&toc_html, &toc_url, &mut book)?;

    let mode_choices = ["全卷下载（生成一个 EPUB）", "分卷下载（可多选）"].map(str::to_owned);
    let split_mode = select_option("请选择下载模式：", &mode_choices, 0)? == 1;
    let selections = if split_mode {
        let volume_choices = book
            .volumes
            .iter()
            .enumerate()
            .map(|(index, volume)| format!("第 {} 卷：{}", index + 1, volume.name))
            .collect::<Vec<_>>();
        select_multiple("请选择要下载的分卷：", &volume_choices)?
            .into_iter()
            .map(Selection::Volume)
            .collect::<Vec<_>>()
    } else {
        vec![Selection::All]
    };

    let cover_source = if split_mode {
        println!("{} 已固定使用当前卷的第一张图片作为封面", success_mark("◆"));
        CoverSource::FirstImage
    } else {
        choose_cover_source()?
    };
    let version = choose_version()?;

    // 任务池大小等于所选卷数，使各分卷真正同时下载。
    let max_jobs = selections.len();
    // 每个分卷都保留原有的章节并发 3、图片并发 5。
    const CHAPTER_CONCURRENCY: usize = 3;
    const IMAGE_CONCURRENCY: usize = 5;
    if split_mode {
        println!(
            "{} {} 个分卷并行（每卷章节并发 {}，图片并发 {}；理论总并发 {}/{}）",
            success_mark("◆"),
            max_jobs,
            CHAPTER_CONCURRENCY,
            IMAGE_CONCURRENCY,
            max_jobs * CHAPTER_CONCURRENCY,
            max_jobs * IMAGE_CONCURRENCY
        );
    }
    let manager = DownloadManager::with_sink(
        max_jobs,
        CHAPTER_CONCURRENCY,
        IMAGE_CONCURRENCY,
        cover_source,
        Arc::new(CliSink),
    )?;
    manager.cache_book(url.clone(), book);

    // 为每个选中的分卷创建独立任务；全卷模式只有一个任务。
    let mut job_ids = Vec::new();
    for selection in selections {
        let job_id = match manager
            .dispatch(Command::CreateJob {
                url: url.clone(),
                selection,
                version,
                title_style,
            })
            .await?
        {
            CommandOutcome::Created(id) => id,
            _ => return Err(Error::new(ErrorKind::Encode, "创建小说失败".into())),
        };
        job_ids.push(job_id);
    }
    for &job_id in &job_ids {
        manager.dispatch(Command::StartJob { job_id }).await?;
    }

    // 每个分卷任务使用独立进度条。
    let mp = MultiProgress::new();
    let mut progress_bars = HashMap::new();
    for &job_id in &job_ids {
        let pb = mp.add(ProgressBar::new(100));
        pb.set_style(
            ProgressStyle::with_template("{msg:30} {bar:40.blue} {pos}%")
                .unwrap()
                .progress_chars("█░"),
        );
        pb.set_message(format!("任务 #{job_id} 准备中"));
        progress_bars.insert(job_id, pb);
    }

    // 轮询快照直到结束
    loop {
        let snapshot = manager.get_snapshot();
        let mut all_done = true;
        for job in &snapshot {
            if let Some(pb) = progress_bars.get(&job.job_id) {
                pb.set_message(format!("任务 #{} {}", job.job_id, stage_message(job)));
                pb.set_position(job.percent as u64);
            }
            if !matches!(
                job.status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            ) {
                all_done = false;
            }
        }

        if all_done {
            for pb in progress_bars.values() {
                pb.finish_and_clear();
            }
            mp.clear()
                .map_err(|e| Error::new(ErrorKind::Encode, format!("清理进度条失败: {e}")))?;
            let mut snapshot = snapshot;
            snapshot.sort_by_key(|job| job.job_id);
            for job in &snapshot {
                match job.status {
                    JobStatus::Completed => {
                        println!(
                            "{} 小说 #{} 完成：{}",
                            success_mark("✔"),
                            job.job_id,
                            success(job.result_path.as_deref().unwrap_or("未知路径"))
                        );
                    }
                    JobStatus::Failed => {
                        println!(
                            "{} 小说 #{} 失败：{}",
                            failure_mark("✘"),
                            job.job_id,
                            failure(job.error.as_deref().unwrap_or("未知错误"))
                        );
                    }
                    JobStatus::Cancelled => {
                        println!("{} 小说 #{} 已取消", cancel_mark("✘"), job.job_id);
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
