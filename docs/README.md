### 文档站点（MkDocs）使用说明

本项目使用 MkDocs 构建文档站点，主题为 Material。以下为最小可用步骤与常用命令。

#### 1. 环境与依赖安装
- Python 3.8+
- 安装依赖（推荐）
```bash
pip install -r docs/requirements.txt
```
- 可选插件（按需）
```bash
pip install markdown-checklist
pip install mkdocs-git-revision-date-localized-plugin
```
（`requirements.txt` 已包含：mkdocs、mkdocs-material、mkdocs-minify-plugin、pymdown-extensions）

#### 2. 主题
- 使用主题：Material（包名：mkdocs-material）
- 在 mkdocs.yml 中声明（示例）：
```yaml
theme:
  name: material
```

#### 3. 启动（本地预览）
在包含 mkdocs.yml 的目录执行：
```bash
mkdocs serve -a 127.0.0.1:8000
```
- 启动命令参数说明：
  - `-a, --dev-addr <host:port>`：绑定地址与端口（缺省为 127.0.0.1:8000）
  - `-f, --config-file <path>`：指定配置文件（默认 `mkdocs.yml`）

访问：http://127.0.0.1:8000/

#### 4. 构建（生成静态站点）
```bash
mkdocs build -d site
```
- 常用参数：
  - `-d, --site-dir <path>`：输出目录（默认 `site/`）
  - `-f, --config-file <path>`：指定配置文件

#### 5. 从零新建站点（可选）
若你需要在空目录初始化：
```bash
mkdocs new mysite
cd mysite
mkdocs serve
```
然后根据需要安装插件：
```bash
pip install markdown-checklist mkdocs-minify-plugin mkdocs-git-revision-date-localized-plugin
```

#### 6. 目录与文件
- `docs/`：放置 Markdown 文档
- `docs/requirements.txt`：MkDocs 及主题/插件依赖清单
- `mkdocs.yml`：站点配置（主题、导航、插件等）

> 提示：若尚未创建 `mkdocs.yml`，请在项目根目录新增并配置主题、导航与插件。

