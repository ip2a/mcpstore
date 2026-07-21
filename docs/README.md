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
