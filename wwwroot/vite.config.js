import { defineConfig } from 'vite'

export default defineConfig({
  root: '.',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false
  },
  server: {
    port: 3000,
    host: true,
    // 配置静态文件服务
    fs: {
      allow: ['.']
    }
  },
  // 确保 public 目录中的文件可以被访问
  publicDir: 'public'
})
