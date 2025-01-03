# 本软件用于下载wenku8.cc的轻小说
## 目前已经实现的功能：
<li>下载并生成epub3文件（可能有一些阅读器不支持）</li>
<li>强制使用本地封面，暂不支持网络获取</li>
<li>使用nodejs本地环境</li>
<li>强制全书下载，不支持分卷</li>

##  未实现的功能
<li>建立良好的封面数据库</li>
<li>分卷</li>
<li>生成epub2</li>
<li>任何错误检查和error抛出，有可能下不了报错吧（doge）</li>

## 使用说明

<ol>
<li>
首先要求有nodejs环境  
建议从nodejs官网下载  
</li>
<li>
获取软件

```
git clone https://github.com/1324762577/wenku8EpubDownloader-Nodejs-.git
```
</li>
<li>
切换到目标文件夹
</li>
<li>
将你自己找到的封面放到文件夹根目录并命名为cover.jpg
<li>
复制wenku8小说网页的url地址，如 

> https://www.wenku8.cc/book/3057.htm   


然后运行以下代码

```(javascript)
node ./index.js
```
输入刚才获得的地址
</li>
<li>
稍等片刻便可在文件夹中看见生成的文件，这样就可以快乐观看了        

 ☆*: .｡. o(≧▽≦)o .｡.:*☆
</li>
</ol>

ps:建议使用wenku8.cc域名，因为wenku8.net我没试过  
(\*/ω＼\*)


