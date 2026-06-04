# Docker 部署说明

## 前置条件

- 已安装 Docker 与 Docker Compose
- 在仓库根目录执行命令

## 按服务启动

```bash
docker compose -f docker/doc/docker-compose.yml up -d
docker compose -f docker/web/docker-compose.yml up -d
docker compose -f docker/api/docker-compose.yml up -d
docker compose -f docker/wiki/docker-compose.yml up -d
```

## 停止服务

```bash
docker compose -f docker/doc/docker-compose.yml down
docker compose -f docker/web/docker-compose.yml down
docker compose -f docker/api/docker-compose.yml down
docker compose -f docker/wiki/docker-compose.yml down
```

## 一键脚本

- 启动全部：`docker/start-all.sh`
- 停止全部：`docker/stop-all.sh`
- 查看状态：`docker/status.sh`

## 目录结构

```text
docker/
├── doc/                    # 文档服务
├── web/                    # 前端服务
├── api/                    # API 服务
├── wiki/                   # Wiki 服务
├── start-all.sh            # 一键启动
├── stop-all.sh             # 一键停止
└── status.sh               # 状态检查
```
