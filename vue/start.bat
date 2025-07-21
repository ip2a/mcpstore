@echo off
echo ========================================
echo MCPStore Vue Frontend 启动脚本
echo ========================================
echo.

:: 检查 Node.js 是否安装
node --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ 错误: 未检测到 Node.js
    echo 请先安装 Node.js (版本 >= 16.0.0)
    echo 下载地址: https://nodejs.org/
    pause
    exit /b 1
)

:: 显示 Node.js 版本
echo ✅ Node.js 版本:
node --version
echo.

:: 检查 npm 是否可用
npm --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ 错误: npm 不可用
    pause
    exit /b 1
)

:: 显示 npm 版本
echo ✅ npm 版本:
npm --version
echo.

:: 检查是否存在 node_modules
if not exist "node_modules" (
    echo 📦 首次运行，正在安装依赖...
    echo.
    npm install
    if %errorlevel% neq 0 (
        echo ❌ 依赖安装失败
        pause
        exit /b 1
    )
    echo ✅ 依赖安装完成
    echo.
)

:: 检查后端服务
echo 🔍 检查后端服务状态...
curl -s http://localhost:18200/for_store/list_services >nul 2>&1
if %errorlevel% neq 0 (
    echo ⚠️  警告: 后端服务 (端口 18200) 似乎未启动
    echo 请确保后端服务正在运行:
    echo python -m mcpstore.cli.main run api --port 18200
    echo.
    echo 是否继续启动前端? (y/n)
    set /p choice=
    if /i "%choice%" neq "y" (
        echo 已取消启动
        pause
        exit /b 0
    )
) else (
    echo ✅ 后端服务运行正常
)
echo.

:: 启动开发服务器
echo 🚀 启动 MCPStore Vue Frontend...
echo 前端地址: http://localhost:5177
echo 后端地址: http://localhost:18200
echo.
echo 按 Ctrl+C 停止服务器
echo ========================================
echo.

npm run dev

pause
