#!/usr/bin/env node

/**
 * Vue环境配置检查脚本
 * 用于验证本地和域名环境的配置是否正确
 */

import fs from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

console.log('🔍 检查Vue环境配置...\n')

// 检查环境文件
const envFiles = [
  { file: '.env', name: '默认环境' },
  { file: '.env.local', name: '本地环境' },
  { file: '.env.domain', name: '域名环境' }
]

console.log('📁 环境文件检查:')
envFiles.forEach(({ file, name }) => {
  const filePath = path.join(__dirname, file)
  if (fs.existsSync(filePath)) {
    console.log(`  ✅ ${name} (${file}) - 存在`)
    
    // 读取并显示关键配置
    const content = fs.readFileSync(filePath, 'utf8')
    const apiUrl = content.match(/VITE_API_BASE_URL=(.+)/)?.[1]
    const devPort = content.match(/VITE_DEV_PORT=(.+)/)?.[1]
    
    if (apiUrl) console.log(`     📡 API地址: ${apiUrl}`)
    if (devPort) console.log(`     🔌 开发端口: ${devPort}`)
  } else {
    console.log(`  ❌ ${name} (${file}) - 缺失`)
  }
})

console.log('\n📦 package.json脚本检查:')
const packagePath = path.join(__dirname, 'package.json')
if (fs.existsSync(packagePath)) {
  const packageJson = JSON.parse(fs.readFileSync(packagePath, 'utf8'))
  const scripts = packageJson.scripts || {}
  
  const requiredScripts = ['dev', 'dev:domain', 'build', 'build:domain']
  requiredScripts.forEach(script => {
    if (scripts[script]) {
      console.log(`  ✅ ${script}: ${scripts[script]}`)
    } else {
      console.log(`  ❌ ${script}: 缺失`)
    }
  })
} else {
  console.log('  ❌ package.json 不存在')
}

console.log('\n⚙️  vite.config.js检查:')
const viteConfigPath = path.join(__dirname, 'vite.config.js')
if (fs.existsSync(viteConfigPath)) {
  console.log('  ✅ vite.config.js 存在')
  
  const viteConfig = fs.readFileSync(viteConfigPath, 'utf8')
  
  // 检查关键配置
  const checks = [
    { pattern: /mode === 'domain'/, name: '域名模式检测' },
    { pattern: /base = isDomain \? '\/web_demo\/' : '\/'/, name: 'base路径配置' },
    { pattern: /hmr:/, name: 'HMR配置' },
    { pattern: /allowedHosts/, name: '允许的主机配置' }
  ]
  
  checks.forEach(({ pattern, name }) => {
    if (pattern.test(viteConfig)) {
      console.log(`  ✅ ${name}`)
    } else {
      console.log(`  ❌ ${name}`)
    }
  })
} else {
  console.log('  ❌ vite.config.js 不存在')
}

console.log('\n🌐 nginx配置检查:')
const nginxConfigPath = path.join(__dirname, '../frpnginx/nginx_mcpstore.conf')
if (fs.existsSync(nginxConfigPath)) {
  console.log('  ✅ nginx配置文件存在')
  
  const nginxConfig = fs.readFileSync(nginxConfigPath, 'utf8')
  
  const nginxChecks = [
    { pattern: /map \$http_upgrade \$connection_upgrade/, name: 'WebSocket升级映射' },
    { pattern: /location \/web_demo/, name: '前端代理配置' },
    { pattern: /proxy_set_header Upgrade/, name: 'WebSocket头部配置' },
    { pattern: /location \/web_demo\/@vite\/client/, name: 'Vite WebSocket专用路径' }
  ]
  
  nginxChecks.forEach(({ pattern, name }) => {
    if (pattern.test(nginxConfig)) {
      console.log(`  ✅ ${name}`)
    } else {
      console.log(`  ❌ ${name}`)
    }
  })
} else {
  console.log('  ❌ nginx配置文件不存在')
}

console.log('\n🚀 启动建议:')
console.log('  📍 本地开发: npm run dev')
console.log('  🌍 域名开发: npm run dev:domain')
console.log('  🔧 构建本地: npm run build')
console.log('  🌐 构建域名: npm run build:domain')

console.log('\n✨ 检查完成!')
