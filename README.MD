# Wenku2epub

轻小说下载工具，用于从 wenku8 网站下载小说并生成 EPUB 电子书。

Rust 编写（v2.0.0 起），JS 旧版请移步 v1.0.0 分支。

## 功能

- 一键下载整本小说并生成 EPUB 电子书
- 支持 EPUB 2 / EPUB 3 两种格式
- 支持小说标题解析选择
- 三种封面选择：网站封面 / 卷首插图 / 本地图片
- 修正了延迟机制，会自动处理失败进行重试

## 下载

在 [Releases](https://github.com/Summerburier/wenku2epub/releases) 页面下载对应系统的可执行文件：

| 系统       | 文件               | 可用状态 |
| ---------- | ------------------ | -------- |
| Windows    | `wenku2epub.exe` | ✅       |
| macOS(x86) | `wenku2epub`     | ✅       |
| macOS(arm) | `wenku2epub`     | ✅       |
| Linux      | `wenku2epub`     | ✅       |

下载后直接双击（Windows）或命令行运行（macOS/Linux）即可，无需安装。

PS：建议还是使用终端运行。Windows运行使用 .\wenku2epub.exe，防止报错

## 使用方法

启动程序后，按提示依次操作：

```
========== wenku2epub 小说下载器 ==========                                                
请输入要下载的小说网址：https://www.wenku8.net/book/3988.htm                               
◆ 解析到书名：玩玩的恋爱关系(玩乐关系)                                                     
请选择书名格式：                                                                           
  1. 完整书名（保留括号，默认）                                                            
  2. 括号中的书名：玩乐关系                                                                
  3. 括号前的书名：玩玩的恋爱关系                                                          
请输入序号 (1/2/3，默认 1)：2                                                              
请选择封面来源：                                                                           
  1. 轻小说文库封面                                                                        
  2. 第一卷的第一张图片                                                                    
  3. 当前目录的 cover.jpg/png 等图片                                                       
请输入序号 (1/2/3，默认 1)：3
请选择 EPUB 版本：
  1. EPUB 2 (toc.ncx)
  2. EPUB 3 (nav.xhtml)
请输入序号 (1/2，默认 2)：2
◆ 小说 #0 已创建：https://www.wenku8.net/book/3988.htm
打包 EPUB                                                                                    ████████████████████████████████████████ 100%                                                ✔ 小说 #0 完成：玩乐关系.epub
```

等待下载完成，生成的 `.epub` 文件就在程序所在目录。

## 常见问题

**应该用哪个网址？**

.net和.cc都可以使用，主要看你的网络环境。.cc建议中国移动用户使用。如果有403不妨两个域名都试下

**想用本地图片当封面？**

选择封面来源 3，并把图片命名为 `cover.jpg`（或 `cover.png`、`cover.jpeg`、`cover.gif`、`cover.webp`、`cover.bmp`）放在程序所在目录即可。

## 自行编译（可选）

需要安装 [Rust](https://www.rust-lang.org/tools/install)：

```bash
cargo build --release
```

编译产物在 `target/release/wenku2epub(.exe)`。
