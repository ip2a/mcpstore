# MCPStore 文档

正式文档只面向 MCPStore 使用者，当前先维护中文版。

## 本地预览

```bash
cd docs
uv run mkdocs serve -f mkdocs.yml
```

访问 `http://127.0.0.1:8000/`。

## 构建

```bash
cd docs
uv run mkdocs build --strict -f mkdocs.yml
```

`产出文档/` 是项目内部方案和过程记录，不属于正式发布文档。

## 当前进度

- [x] 中文使用者文档骨架
- [x] 以“管理 MCP 服务”为主线
- [x] 预留本地、远程、CLI、TUI、桌面端、Web、Rust、Python 使用路径
- [x] 补充 CLI、TUI、Web、远程 API、Python 的真实基础流程
- [ ] 补充英文目录
