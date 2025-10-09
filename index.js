const axios = require('axios');
const iconv = require('iconv-lite');
const JsZip = require('jszip');
const cheerio = require('cheerio');
const fs = require('fs');
const prompt = require('prompt-sync')();
const { DOMParser, XMLSerializer } = require('@xmldom/xmldom');
const xmlFormatter = require('xml-formatter');
const beautify = require('js-beautify').html;

// 引入 p-limit 控制并发（需提前安装：npm install p-limit）
const { default: pLimit } = require('p-limit');

// 添加延迟函数，用于控制请求频率
async function delay(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// 随机 User-Agent 列表，提高反爬绕过能力
function getRandomUserAgent() {
  const uas = [
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0',
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15',
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36'
  ];
  return uas[Math.floor(Math.random() * uas.length)];
}

// 定义最大并发请求数（图片下载）
const imgLimit = pLimit(3);

// container.xml 文件内容
const container_xml = `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
   </rootfiles>
</container>
`;

// OPF 文件内容
const content_opf = `<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="PrimaryID">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title></dc:title>
    <dc:identifier opf:scheme="ISBN"/>
    <dc:language>zh-CN</dc:language>
    <dc:creator></dc:creator>
    <dc:description></dc:description>
  </metadata>
  <manifest>
  </manifest>
  <spine toc="ncx">
  </spine>
</package>
`;

// XHTML 文件模板
const content_xhtml = `<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title></title>
  <link rel="stylesheet" type="text/css" href="../Style/style.css" />
</head>
<body>
  <section>
    <h3></h3>
  </section>
</body>
</html>
`;

// nav.xhtml 导航文件内容
const nav_xhtml = `<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="zh-CN" xml:lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <title>ePub Nav</title>
    <style type="text/css">
      ol { list-style-type: none; margin: 0; padding: 0; }
      li { margin: 0.2em 0; }
    </style>
  </head>
  <body epub:type="frontmatter">
    <nav epub:type="toc" id="toc">
    </nav>
  </body>
</html>`;

// 获取网页内容的函数（带延迟和随机 UA）
async function ask(url) {
  await delay(1000); // 每次请求间隔 1 秒
  try {
    const response = await axios.get(url, {
      responseType: 'arraybuffer',
      headers: {
        'User-Agent': getRandomUserAgent(),
        'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6',
        'Accept-Encoding': 'gzip, deflate, br',
        'Connection': 'keep-alive',
        'Referer': 'https://www.wenku8.net/',
        'DNT': '1',
        'Upgrade-Insecure-Requests': '1',
        'Sec-Fetch-Dest': 'document',
        'Sec-Fetch-Mode': 'navigate',
        'Sec-Fetch-Site': 'same-origin',
        'Sec-Fetch-User': '?1'
      },
      timeout: 10000
    });
    const decodedData = iconv.decode(Buffer.from(response.data), 'GBK');
    return cheerio.load(decodedData);
  } catch (error) {
    console.error(`Error fetching the URL ${url}:`, error.message);
    return null;
  }
}

// 获取小说信息
async function getBookInfo(url, json) {
  const $ = await ask(url);
  if ($) {
    let titles, authors, intro, chapurl;
    const patt = new RegExp('(.*)\\x20-\\x20(.*)\\x20-\\x20(.*)\\x20-\\x20(.*)');
    const match = patt.exec($('title').text());
    if (match) {
      titles = match[1];
      authors = match[2];
    }

    $('span[style="font-size:14px;"]').each((i, elem) => {
      intro = $(elem).text().trim();
    });

    $('a').each((i, elem) => {
      if ($(elem).text().trim() === '小说目录') {
        const href = $(elem).attr('href');
        if (href) {
          chapurl = new URL(href, url).href;
        }
      }
    });

    json.titles = titles;
    json.authors = authors;
    json.intro = intro || '暂无简介';
    json.content = {};
    if (chapurl) {
      await getChapList(chapurl, json);
    }
  }
}

// 获取章节列表
async function getChapList(url, json) {
  const $ = await ask(url);
  if ($) {
    let key;
    let p = 0;
    let v = -1;
    const patt = /(.*)index\.htm/;
    const realur = patt.exec(url)[1];

    $('td').each((i, elem) => {
      const $elem = $(elem);
      if ($elem.attr('class') === 'vcss') {
        v++;
        key = $elem.text().trim();
        json.content[v] = { volume: key, chapters: {} };
        p = 0;
      } else if ($elem.attr('class') === 'ccss' && $elem.find('a').length > 0) {
        const link = $elem.find('a').first();
        const title = link.text().trim();
        const href = realur + link.attr('href');
        json.content[v].chapters[p] = { title, href };
        p++;
      }
    });
  }
}

// 下载并保存图片
async function getImg(src, volume, chapter, j, book) {
  const imgname = `${volume}_${chapter}_${j}.jpg`;
  const imgpath = `OEBPS/Image/${imgname}`;
  try {
    const response = await axios.get(src, {
      responseType: 'arraybuffer',
      headers: {
        'User-Agent': getRandomUserAgent(),
        'Referer': 'https://www.wenku8.net/',
        'Accept': 'image/webp,image/apng,image/*,*/*;q=0.8'
      },
      timeout: 10000
    });
    book.file(imgpath, response.data);
    console.log(`Image ${imgname} downloaded`);
  } catch (error) {
    console.error(`Failed to download image ${src}:`, error.message);
  }
}

// 创建文本内容并添加到 EPUB 中
async function creatText(book, json) {
  const parser = new DOMParser();
  const serializer = new XMLSerializer();
  json.imgs = {};
  let imgcount = 0;

  for (const volume in json.content) {
    for (const chapter in json.content[volume].chapters) {
      const { title, href } = json.content[volume].chapters[chapter];
      const $ = await ask(href);
      if (!$) continue;

      const content = $('#content');
      if (!content.length) continue;

      const xhtml = parser.parseFromString(content_xhtml, 'application/xhtml+xml');
      xhtml.getElementsByTagName('title')[0].textContent = title;
      xhtml.getElementsByTagName('h3')[0].textContent = title;

      content.contents().each((index, element) => {
        if (element.type === 'text' && element.data.trim() !== '') {
          const p = xhtml.createElement('p');
          p.textContent = $(element).text().trim();
         xhtml.getElementsByTagName('section')[0].appendChild(p);
        }
      });

      const imgs = content.find('img');
      const imgPromises = [];
      imgs.each((j, img) => {
        const src = $(img).attr('src');
        if (!src) return;
        const absSrc = new URL(src, href).href;
        const imgname = `${volume}_${chapter}_${imgcount}.jpg`;
        json.imgs[imgcount] = { imgname };

        const imgTag = xhtml.createElement('img');
        imgTag.setAttribute('src', `../Image/${imgname}`);
        xhtml.getElementsByTagName('section')[0].appendChild(imgTag);

        imgPromises.push(
          imgLimit(() => getImg(absSrc, volume, chapter, imgcount++, book))
        );
      });

      await Promise.all(imgPromises);

      const formattedXhtml = beautify(serializer.serializeToString(xhtml), { indent_size: 2 });
      book.file(`OEBPS/Text/${volume}_${chapter}.xhtml`, Buffer.from(iconv.encode(formattedXhtml, 'utf-8')));
      console.log(`Chapter ${volume}_${chapter} processed`);
    }
  }
}

// 创建 OPF 文件
async function creatOpf(book, json) {
  const parser = new DOMParser();
  const opf = parser.parseFromString(content_opf, 'text/xml');

  opf.getElementsByTagName('dc:title')[0].textContent = json.titles;
  opf.getElementsByTagName('dc:creator')[0].textContent = json.authors;
  opf.getElementsByTagName('dc:description')[0].textContent = json.intro;

  // 添加封面、CSS、nav 等资源
  const manifest = opf.getElementsByTagName('manifest')[0];
  const spine = opf.getElementsByTagName('spine')[0];

  const addItem = (id, href, mediaType, properties = null) => {
    const item = opf.createElement('item');
    item.setAttribute('id', id);
    item.setAttribute('href', href);
    item.setAttribute('media-type', mediaType);
    if (properties) item.setAttribute('properties', properties);
    manifest.appendChild(item);
  };

  addItem('cover', 'Image/cover.jpg', 'image/jpeg');
  addItem('style.css', 'Style/style.css', 'text/css');
  addItem('nav', 'nav.xhtml', 'application/xhtml+xml', 'nav');

  // 添加章节和图片
  for (const volume in json.content) {
    for (const chapter in json.content[volume].chapters) {
      addItem(`Text/${volume}_${chapter}.xhtml`, `Text/${volume}_${chapter}.xhtml`, 'application/xhtml+xml');
      const itemref = opf.createElement('itemref');
      itemref.setAttribute('idref', `Text/${volume}_${chapter}.xhtml`);
      spine.appendChild(itemref);
    }
  }

  for (const i in json.imgs) {
    const { imgname } = json.imgs[i];
    addItem(`Image/${imgname}`, `Image/${imgname}`, 'image/jpeg');
  }

  const formattedOpf = xmlFormatter(new XMLSerializer().serializeToString(opf), { indentation: '  ' });
  book.file('OEBPS/content.opf', formattedOpf);
}

// 创建导航文件 nav.xhtml
async function creatNav(book, json) {
  const parser = new DOMParser();
  const serializer = new XMLSerializer();
  const nav = parser.parseFromString(nav_xhtml, 'text/xml');
  const ol = nav.createElement('ol');

  for (const volume in json.content) {
    const volData = json.content[volume];
    const li = nav.createElement('li');
    const a = nav.createElement('a');
    a.setAttribute('href', `Text/${volume}_0.xhtml`);
    a.textContent = volData.volume;
    li.appendChild(a);

    const nestedOl = nav.createElement('ol');
    for (const chapter in volData.chapters) {
      const chData = volData.chapters[chapter];
      const nestedLi = nav.createElement('li');
      const nestedA = nav.createElement('a');
      nestedA.setAttribute('href', `Text/${volume}_${chapter}.xhtml`);
      nestedA.textContent = chData.title;
      nestedLi.appendChild(nestedA);
      nestedOl.appendChild(nestedLi);
    }
    li.appendChild(nestedOl);
    ol.appendChild(li);
  }

  nav.getElementById('toc').appendChild(ol);
  const formattedNav = beautify(serializer.serializeToString(nav), { indent_size: 2 });
  book.file('OEBPS/nav.xhtml', Buffer.from(iconv.encode(formattedNav, 'utf-8')));
}

// 创建完整 EPUB 压缩包
async function creatEpub(json) {
  const book = new JsZip();
  const coverPath = './cover.jpg';

  book.file('mimetype', 'application/epub+zip', { compression: 'STORE' });
  book.folder('META-INF').file('container.xml', container_xml);
  book.folder('OEBPS');
  book.folder('OEBPS/Image');
  book.folder('OEBPS/Text');
  book.folder('OEBPS/Style');

  // 添加封面和样式
  if (fs.existsSync(coverPath)) {
    book.file('OEBPS/Image/cover.jpg', fs.readFileSync(coverPath));
  } else {
    console.warn('未找到封面图片 cover.jpg');
  }

  if (fs.existsSync('style.css')) {
    book.file('OEBPS/Style/style.css', fs.readFileSync('style.css'));
  }

  await creatNav(book, json);
  await creatText(book, json);
  await creatOpf(book, json);

  return book;
}

// 主入口函数
async function scraper(url) {
  const json = {};
  await getBookInfo(url, json);
  if (!json.titles) {
    console.error('未能获取书籍标题，可能网址无效或页面结构变化。');
    return;
  }

  const book = await creatEpub(json);
  return book.generateAsync({ type: 'nodebuffer' }).then(content => {
    const filename = `${json.titles}.epub`.replace(/[<>:"/\\|?*]/g, '_'); // 清理非法文件名字符
    fs.writeFileSync(filename, content);
    console.log(`EPUB 文件已生成：${filename}`);
  });
}

// 启动交互式输入
const url = prompt('请输入要下载的小说网址：');
if (url) {
  scraper(url).catch(err => console.error('程序出错：', err));
}