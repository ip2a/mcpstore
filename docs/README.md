
```bash
uv add  mkdocs mkdocs-material mkdocs-minify-plugin pymdown-extensions
```


```bash
mkdocs serve -a 127.0.0.1:8000
```

访问 `http://127.0.0.1:8000/`。

## 3. 构建静态站点

```bash
uv run mkdocs build -f docs/mkdocs.yml
```


