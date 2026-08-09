# wenku2epub 

轻小说下载工具，用于将 wenku8.cc 的轻小说下载并生成 EPUB3 文件 (oﾟvﾟ)ノ

## 功能 (｡･ω･)ﾉﾞ

- 下载并生成 EPUB3 文件
- 强制使用本地封面
- 强制全书下载

## 未实现的功能 (´・ω・`)

- 建立良好的封面数据库
- 分卷下载
- 生成 EPUB2
- 错误检查（有可能下不了报错吧 d(￣ ￣)）

## 使用说明 (｀・ω・´)

### 1. 安装 Node.js

从 [Node.js 官网](https://nodejs.org/) 下载并安装 Node.js。

### 2. 获取代码 

```bash
git clone https://github.com/Summerburier/wenku2epub.git
```
> ps: 如果有GitHub账号可以使用 SSH 克隆：
```bash
git clone git@github.com:Summerburier/wenku2epub.git
```

```bash
cd wenku2epub
```

### 3. 安装依赖 

```bash
npm install
```

### 4. 准备封面 

将你的封面图片放到项目根目录，并命名为 `cover.jpg`。

### 5. 运行 ヽ(✿ﾟ▽ﾟ)ノ

```bash
npm run start
```

输入 wenku8 小说网页的 URL 地址（如 `https://www.wenku8.cc/book/3057.htm`），稍等片刻即可在文件夹中找到生成的 EPUB 文件 (≧▽≦)/

> 建议使用 wenku8.cc 域名，wenku8.net 经过测试和cc的效果差不多，但是针对中国移动用户可能无法访问。
---
*如果觉得好用，点个 star 支持一下吧！* ☆*: .｡. o(≧▽≦)o .｡.:*☆
