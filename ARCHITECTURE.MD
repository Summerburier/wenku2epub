# wenku2epub 架构设计（Rust 版）

## 项目思路

wenku8轻小说文库原生不提供epub格式下载，只提供了TXT, UMD, JAR, JAD这4种格式，均为纯文本，不包含任何图片。其中JAR格式我是用软件查看并不能正常读取图片（可能是软件问题）

唯一结构清晰，内容完整且含有图片的来源是wenku8的网页阅读部分，其中插图单独开了一章，名为插图。所以最方便的构建epub电子书的方式是对wenku8的网页直接进行解析，爬取对应文本和图片来构建电子书。

详细的EPUB3.x标准格式请参考[EPUB 3.3](https://www.w3.org/TR/epub-33/)

详细的EPUB2.x标准格式请参考[EPUB2.0.1](https://idpf.org/epub/20/spec/OPS_2.0_latest.htm)

wenku8轻小说文库可获取的基本信息有：书名，作者，文库，封面，简介，卷名，章节名等。

---

## 设计原则：核心与 UI 完全解耦

本项目后期将使用 **Flutter + flutter_rust_bridge** 编写 GUI，同时保留 **CLI** 供快速使用。本文档只设计 **core 下载核心** 与其两端客户端的桥接方式，不做 UI 具体设计。

```
┌───────────────────────────────┐
│  Flutter UI（未来，另行设计）   │        CLI（app crate，thin 客户端）
└──────────────┬────────────────┘   ┌─────▲───────────────┘
               │ flutter_rust_bridge │    │ 实现 DownloaderEvent trait
               │ (FFI, 编译为 cdylib) │    │ (dialoguer + indicatif 展示)
┌──────────────▼────────────────┐   │    │
│            core 库 (下载核心)   │◄──┘    │
│  不含任何 UI/终端代码           │◄───────┘
└───────────────────────────────┘
```

core 保持**零 UI 依赖**，通过两种方式被消费：
- **Rust 内部（CLI）**：实现事件 trait 回调
- **Flutter（未来）**：编译为 cdylib，通过 flutter_rust_bridge FFI 调用

### 核心原则

1. **core 不 import 任何 UI 相关库**（如 dialoguer、indicatif、终端输出），这些都属于 UI 层
2. **下载器是一个独立类（struct）**，实例化、配置、运行、销毁都独立，UI 只负责调用它
3. **进度/状态通过事件流上抛**：core 通过 `async_stream` 或 channel 暴露进度事件，由 flutter_rust_bridge 自动映射为 Dart Stream
4. **配置可注入**：请求并发数、是否延迟、User-Agent 列表、重试次数等由调用方传入，不硬编码
5. **数据模型与解析/打包分离**：模型是纯数据结构，解析和打包只操作模型，互不依赖
6. **core 编译为 cdylib**，通过 flutter_rust_bridge 对外暴露，供 Flutter 直接 FFI 调用

---

## 项目结构（core + CLI）

```
wenku2epub/
├── core/                     # 下载核心库（无 UI 依赖，crate-type=["lib","cdylib"]）
│   └── src/
│       ├── lib.rs            # 公开 API 出口 + 代码生成入口
│       ├── downloader.rs     # Downloader 类：流程编排
│       ├── event.rs          # 进度事件模型（trait + Stream，供 FFI 映射）
│       ├── client.rs         # HTTP 层：请求/重试/编码
│       ├── parser.rs         # 解析层：书页/目录/章节
│       ├── model.rs          # 纯数据模型
│       └── epub.rs           # EPUB 打包
├── app/                      # CLI（thin 客户端，依赖 core）
│   └── src/
│       └── main.rs           # 输入收集 + 实现 DownloaderEvent + 进度展示
└── flutter_rust_bridge.yaml  # 桥接代码生成配置（未来）
```

`Cargo.toml` 采用 workspace：`core` 是库 crate（`crate-type=["lib","cdylib"]`），`app` 是二进制 crate，依赖 `core`。

---

## 模块设计规范（参考 SPlayer-Next）

参考成熟项目的模式，core 内部采用**逻辑层 + 绑定层**双层分离，确保纯逻辑可独立测试、可被多端复用。

### 双层分离

```
core/src/
├── downloader.rs     # 逻辑层：Downloader（纯 Rust，不含 frb/napi 代码）
├── bindings.rs       # 绑定层（未来 frb）：#[frb] 导出，转发逻辑层方法
├── event.rs          # 事件枚举 + EventEmitter（纯 Rust）
└── ...               # client / parser / model / epub 均为纯逻辑
```

- **逻辑层**：纯 Rust struct，字段是状态，方法操作状态，不依赖任何跨语言工具，可独立单元测试
- **绑定层**：只在未来加，用 frb 宏标记，持有一个逻辑层实例，把方法转发给 Flutter，并负责错误转换与 Stream 映射

### 类（struct）的声明规范

1. **一个 struct 集中所有状态字段**，每个字段一行注释说明用途
2. **`impl` 块内所有方法都有 `///` 文档注释**，说明行为、副作用、边界条件
3. **逻辑层方法签名用 Rust 原生类型**（`String`、`f64`、`Option<T>`），不用任何绑定层类型

```rust
/// 下载器：流程编排，纯逻辑层
pub struct Downloader {
    config: DownloaderConfig,      // 注入的配置
    event_callback: Option<EventEmitter>, // 进度事件回调
    img_counter: AtomicU64,        // 全局图片计数器（跨卷章递增）
}

impl Downloader {
    /// 执行完整下载流程，返回生成的 EPUB 路径
    pub async fn run(&mut self, url: &str) -> Result<String, DownloadError> { ... }

    /// 取消任务
    pub fn cancel(&mut self) { ... }
}
```

### 事件规范

事件枚举定义在逻辑层，用 `Arc<闭包>` 做回调；逻辑层只调用 `self.emit(event)`，不关心事件如何传给前端。

```rust
pub enum Event {
    StartBook,
    StartParseToc,
    ParseTocDone { volumes: usize, chapters: usize },
    ChapterDone { current: usize, total: usize },
    ImageDone { current: usize, total: usize },
    BookDone { path: String },
    Error { message: String },
}

pub type EventEmitter = Arc<dyn Fn(Event) + Send + Sync>;
```

绑定层（frb）负责把 `Event` 映射为 Dart Stream；CLI 实现事件 trait 直接消费。

### 错误处理规范

- 逻辑层返回强类型错误：`ParserError` / `HttpError` / `DownloadError`
- 逻辑层不依赖绑定层错误类型
- 绑定层负责把错误转换为前端可识别的形式（如带 code 的错误消息）

---

## 核心 API 设计（Downloader 类）

`core` 对外暴露一个 `Downloader`，调用方只需三步：**创建 → 配置 → 运行**。

### 事件（Event）双通道

进度事件同时支持两种消费方式：

```text
通道 1：trait 回调（Rust 内部 / CLI 用）
        UI 实现 DownloaderEvent trait，core 直接调用

通道 2：Stream 上抛（Flutter 用）
        core 通过 async Stream 暴露事件
        flutter_rust_bridge 自动映射为 Dart Stream
```

事件列表：
- StartBook       书名开始
- StartParseToc   开始解析目录
- ParseTocDone    目录解析完成（卷数/章节数）
- ChapterProgress 单章进度（当前/总数，或百分比）
- ChapterDone     单章完成
- ImageProgress   图片进度
- ImageDone       单张图片完成
- BookDone        EPUB 生成完成（输出路径）
- Error           错误信息
```

### Downloader 行为

```text
Downloader::new(config)      // 传入配置（并发数、延迟开关、UA 列表等）
Downloader::run(url)         // 异步执行完整下载流程，返回 Result
Downloader::cancel()         // 取消任务（供 UI 的"停止"按钮调用）
Downloader::events()         // 返回进度事件 Stream
```

`run()` 内部流程（沿用 JS 版逻辑）：

```
解析书页(书名/作者/简介/封面/目录链接)
  → 解析目录(卷/章节/链接) → 生成并保存目录模型
  → 逐章抓取正文+图片（章节串行，图片并发）
  → 生成 EPUB(container/opf/nav/xhtml/图片) → 输出文件
```

### 数据模型（model.rs）

```text
Book      书名/作者/简介/封面字节/卷列表/图片表
Volume    卷名/章节列表
Chapter   标题/URL/解析后 XHTML/图片引用
Image     URL/文件名/字节数据
```

---

## core 内部模块职责

### client.rs — HTTP 层

- 封装 `reqwest`，提供带重试的请求函数
- 处理：随机 UA、Referer 动态生成、429 限流重试（读 Retry-After）、超时
- 返回原始 bytes，由解析层负责编码解码

### parser.rs — 解析层

- 书页解析：标题/作者/简介/目录链接
- 目录解析：卷/章节/跳转链接
- 章节解析：正文文本 + 图片标签
- 依赖 `scraper`（HTML DOM）+ `encoding_rs`（GBK 解码）

### epub.rs — 打包层

- 生成 container.xml / content.opf / nav.xhtml / 章节 xhtml
- 写入图片、封面、style.css
- 用 `zip` crate 打包，mimetype 条目 STORE（不压缩）且必须第一个

### downloader.rs — 流程编排

- 串联上述各层，持有配置与进度事件 Stream
- 负责并发控制（章节串行，图片信号量并发）
- 负责全局图片计数器（跨卷章递增命名）

### event.rs — 事件模型

- 定义进度事件枚举（上述 Event 列表）
- 提供 trait 回调（CLI 用）与 Stream 上抛（Flutter 用）两种接口

---

## app/ — CLI 层

CLI 是 core 的 thin 客户端，只做三件事，不包含任何下载逻辑：

- 收集输入（用 `dialoguer`：URL、是否延迟、并发数）
- 实现 `DownloaderEvent` trait，接收 core 进度事件
- 用 `indicatif` 画进度条展示

依赖 core，逻辑完全复用，仅新增 UI 展示相关库（dialoguer、indicatif 等，均在 app 层而非 core）。

---

## 并发模型

- 章节：串行（XHTML 文件名有顺序依赖，沿用 JS 的 chapterLimit(1)）
- 图片：信号量控制并发 3
- 取消：core 内部用 `CancellationToken`，run 循环中检查，调用 cancel() 即可中断

---

## 错误处理

- 解析层返回强类型错误（`ParserError`：页面结构变化/编码失败）
- HTTP 层区分可重试错误（429/5xx）与致命错误
- 对外统一 `Result<T, DownloadError>`，UI 层只需展示 `Error` 事件

---

## 与 Flutter 的桥接（未来）

core 通过 flutter_rust_bridge 暴露为 FFI：

1. `Downloader::new` / `run` / `cancel` → 映射为 Dart 的 async 方法
2. `Downloader::events`（Stream）→ 映射为 Dart 的 Stream，UI 订阅刷新
3. `Book` / `Volume` / `Chapter` 等模型 → 自动生成 Dart 强类型类

CLI（app）与 Flutter 共享同一套 core，测试也直接针对 core 编写。
Flutter 侧 UI 设计不在本文档范围内，另行规划。
