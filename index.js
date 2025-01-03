const axios = require('axios');
const iconv = require('iconv-lite');
const JsZip = require('jszip');
const cheerio = require('cheerio');
const fs = require('fs');
let readline = require("readline");
const { DOMParser, XMLSerializer } = require('xmldom');
const xmlFormatter = require('xml-formatter');
const beautify = require('js-beautify').html;
//container.xml文件内容
const container_xml = `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
   </rootfiles>
</container>
`;
//OPF文件内容
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
//XHTML文件内容
const content_xhtml = `<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title></title>
  <style type="text/css" src="../Style/style.css"></style>
</head>
<body>
  <section>
    <h3></h3>
  </section>
</body>
</html>
`;
// nav.xhtml 文件内容
const nav_xhtml = `
<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="zh-CN" xml:lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <title>ePub Nav</title>
    <style type="text/css">
    ol { list-style-type: none; }
    </style>
  </head>
  <body epub:type="frontmatter">
    <nav epub:type="toc" id="toc">
    </nav>
  </body>
</html>`;


// 获取网页内容的函数
async function ask(url) {
  try {
    const response = await axios.get(url, {
        responseType: 'arraybuffer',
        headers: {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3',
            'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6',
            'Accept-Encoding': 'gzip, deflate, br',
            'Connection': 'keep-alive',
            'Referer': 'https://www.google.com/',
            'DNT': '1', // Do Not Track Request Header
            'Upgrade-Insecure-Requests': '1',
            'Sec-Fetch-Dest': 'document',
            'Sec-Fetch-Mode': 'navigate',
            'Sec-Fetch-Site': 'none',
            'Sec-Fetch-User': '?1'
          }});
    // 将响应数据转换为GBK编码的字符串
    const decodedData = iconv.decode(Buffer.from(response.data), 'GBK');
    return cheerio.load(decodedData);
  
  } catch (error) {
    console.error(`Error fetching the URL: ${error}`);
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

    json.titles = titles; // 标题
    json.authors = authors; // 作者
    json.intro = intro; // 简介
    json.content = {}; // 章节内容
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
    const patt = new RegExp('(.*)index.htm');
    const realur = patt.exec(url)[1];
    $('td').each((i, elem) => {
        if($(elem).attr('class') === 'vcss') {
            v++;
            key = $(elem).text().trim();
            json.content[v] = { volume: key, chapters: {} };
            p = 0;
        } else if($(elem).attr('class') === 'ccss') {
          if($(elem).find('a').length) {
            json.content[v].chapters[p] = {};
            const link = $(elem).find('a').first();
            if (link) {
                const title = link.text().trim();
                const href = realur + link.attr('href');
                json.content[v].chapters[p] = { title, href };
                p++;
            }}
    
  }
});
}
}

// 创建文本内容并添加到 EPUB 文件中
async function creatText(book, json) {
  const parser = new DOMParser();
  const serializer = new XMLSerializer();
  json.imgs = {};
  let imgcount = 0;
  for (let key in json.content) {
    let volume = key;
    for (let i in json.content[key].chapters) {
      let title = json.content[key].chapters[i].title;
      let href = json.content[key].chapters[i].href;
      let chapter = i;
      let $ = await ask(href);
      if ($) {
        let content = $('div[id="content"]');
        if (content.length) {
          const xhtml = parser.parseFromString(content_xhtml, 'application/xhtml+xml');
          xhtml.getElementsByTagName('title')[0].textContent = title;
          xhtml.getElementsByTagName('h3')[0].textContent = title;
          content.contents().each((index, element) => {
            if (element.type === 'text'&&element.data.trim()!=='') {
              
              let p = xhtml.createElement('p');
              p.textContent = $(element).text().trim();
              xhtml.getElementsByTagName('section')[0].appendChild(p);
            } else if (element.tagName === 'br') {
              // 忽略 <br> 标签
            } else {
              
            }
          });
          let imgs = content.find('img'); //获取图片
          let imgPromises = [];
          imgs.each(async (j, img) => {
            let src = $(img).attr("src");
            let imgname = `${volume}_${chapter}_${j}.jpg`;
            json.imgs[imgcount] = { imgname };
            imgcount++;
            let imgtag = xhtml.createElement('img');
            imgtag.setAttribute("src", `../Image/${imgname}`);
            xhtml.getElementsByTagName('section')[0].appendChild(imgtag);
            imgPromises.push(getImg(src, volume, chapter, j, book));
          });
          await Promise.all(imgPromises);
        
        
        // 格式化 XHTML 内容
        const formattedXhtml = beautify(serializer.serializeToString(xhtml), { indent_size: 2 });

        book.file(`OEBPS/Text/${volume}_${chapter}.xhtml`, Buffer.from(iconv.encode(formattedXhtml, 'utf-8')));
        }
      }
    }
  }
}

// 获取图像数据并保存到本地
async function getImg(src, volume, chapter, j, book) {
  let imgname = `${volume}_${chapter}_${j}.jpg`;
  let imgpath = `OEBPS/Image/${imgname}`;
  try{
  const response = await axios.get(src, {
    responseType: 'arraybuffer',
    headers: {
      'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7',
      'Accept-Encoding': 'gzip, deflate, br, zstd',
      'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6',
      'Cache-Control': 'max-age=0',
      'Priority': 'u=0, i',
      'Sec-CH-UA': '"Microsoft Edge";v="131", "Chromium";v="131", "Not_A Brand";v="24"',
      'Sec-CH-UA-Mobile': '?0',
      'Sec-CH-UA-Platform': '"Windows"',
      'Sec-Fetch-Dest': 'document',
      'Sec-Fetch-Mode': 'navigate',
      'Sec-Fetch-Site': 'none',
      'Sec-Fetch-User': '?1',
      'Upgrade-Insecure-Requests': '1',
      'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0'
    }
  });
  book.file(imgpath, response.data);
  console.log(`Image ${imgname} downloaded`);
}catch (error) {
    console.error(`Error fetching the image: ${error}`);
  }
}



// 创建 OPF 文件
async function creatOpf(book, json) {
  let parser = new DOMParser();
  let opf = parser.parseFromString(content_opf, "text/xml");
  opf.getElementsByTagName("dc:title")[0].textContent = json.titles;
  opf.getElementsByTagName("dc:creator")[0].textContent = json.authors;
  opf.getElementsByTagName("dc:description")[0].textContent = json.intro;
  let itemCover = opf.createElement('item');
  itemCover.setAttribute("id", "cover");
  itemCover.setAttribute("href", "Image/cover.jpg");
  itemCover.setAttribute("media-type", "image/jpeg");
  opf.getElementsByTagName('manifest')[0].appendChild(itemCover);
  let itemcss = opf.createElement('item');
  itemcss.setAttribute("id", "style.css");
  itemcss.setAttribute("href", "Style/style.css");
  itemcss.setAttribute("media-type", "text/css");
  opf.getElementsByTagName('manifest')[0].appendChild(itemcss);
  let itemttf = opf.createElement('item');
  itemttf.setAttribute("id", "Pingfang Regular.ttf");
  itemttf.setAttribute("href", "Font/PingFang Regular.ttf");
  itemttf.setAttribute("media-type", "font/ttf");
  opf.getElementsByTagName('manifest')[0].appendChild(itemttf);
  let item = opf.createElement('item');
  item.setAttribute("id", "nav");
  item.setAttribute("href", "nav.xhtml");
  item.setAttribute("media-type", "application/xhtml+xml");
  item.setAttribute("properties", "nav");
  opf.getElementsByTagName('manifest')[0].appendChild(item);
 
  for (let key in json.content) {
    let volume = key; 
    for (let i in json.content[key].chapters) {
      let chapter = i;
      let item = opf.createElement('item');
      item.setAttribute("id", `Text/${volume}_${chapter}.xhtml`);
      item.setAttribute("href", `Text/${volume}_${chapter}.xhtml`);
      item.setAttribute("media-type", "application/xhtml+xml");
      opf.getElementsByTagName('manifest')[0].appendChild(item);
      let itemref = opf.createElement('itemref');
      itemref.setAttribute("idref", `Text/${volume}_${chapter}.xhtml`);
      opf.getElementsByTagName('spine')[0].appendChild(itemref);
    }
  }
  for (let i in json.imgs) {
    let img = json.imgs[i];
    let item = opf.createElement('item');
    item.setAttribute("id", `Image/${img.imgname}`);
    item.setAttribute("href", `Image/${img.imgname}`);
    item.setAttribute("media-type", "image/jpeg");
    opf.getElementsByTagName('manifest')[0].appendChild(item);
  }

  // 格式化 OPF 内容
  const formattedOpf = xmlFormatter(new XMLSerializer().serializeToString(opf), { indentation: '  ' });
  book.file("OEBPS/content.opf", formattedOpf);
}
// 创建 nav.xhtml 文件
async function creatNav(book, json) {
  let parser = new DOMParser();
  let serializer = new XMLSerializer();
  let nav = parser.parseFromString(nav_xhtml, "text/xml");
  let ol = nav.createElement("ol");
  for (let key in json.content) {
    let volume = key;
    let li = nav.createElement("li");
    let a = nav.createElement("a");
    a.setAttribute("href", `Text/${volume}_0.xhtml`);
    a.textContent = json.content[key].volume;
    li.appendChild(a);
    let nestedOl = nav.createElement("ol");
    for (let i in json.content[key].chapters) {
      let chapter = i;
      let nestedLi = nav.createElement("li");
      let nestedA = nav.createElement("a");
      nestedA.setAttribute("href", `Text/${volume}_${chapter}.xhtml`);
      nestedA.textContent = json.content[key].chapters[i].title;
      nestedLi.appendChild(nestedA);
      nestedOl.appendChild(nestedLi);
    }
    li.appendChild(nestedOl);
    ol.appendChild(li);
  }
  nav.getElementById("toc").appendChild(ol);
  const formattedNav = beautify(serializer.serializeToString(nav), { indent_size: 2 });
  book.file("OEBPS/nav.xhtml", Buffer.from(iconv.encode(formattedNav, 'utf-8')));


}

// 创建 EPUB 文件
async function creatEpub(json) {
  let book = new JsZip();
  let img = fs.readFileSync('..\\cover.jpg');
  book.file("mimetype", "application/epub+zip");
  book.folder("META-INF");
  book.file("META-INF/container.xml", container_xml);
  book.folder("OEBPS");
  book.folder("OEBPS/Image");
  book.file("OEBPS/Image/cover.jpg",img)  ;
  book.folder("OEBPS/Text");
  book.folder("OEBPS/Style");
  book.folder("OEBPS/Font");
  book.file("OEBPS/Style/style.css", fs.readFileSync('style.css'));
  book.file("OEBPS/Font/PingFang Regular.ttf", fs.readFileSync('.\\Pingfang\\PingFang Regular_0.ttf'));
  await creatNav(book, json);
  await creatText(book, json);
  await creatOpf(book, json);
  return book;
}


// 导出主函数
async function scraper(url) {
  let json ={} ;
  let name="";
  await  getBookInfo(url, json);
  name = json.titles;

  console.log(JSON.stringify(json, null, 2));
  let book = await creatEpub(json);
  book.generateAsync({ type: "nodebuffer" })
    .then(function (content) {
      fs.writeFileSync(`./${name}.epub`, content);
      console.log('EPUB 文件已生成');
    });
}

// 调用主函数
const url = 'https://www.wenku8.cc/book/3057.htm'; // 示例 URL
scraper(url);
// 引入readline模块



