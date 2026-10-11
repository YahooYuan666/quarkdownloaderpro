// 开发预览：无 Tauri 运行时（普通浏览器）时安装 IPC 模拟，便于校对界面。
// 产线构建中 import.meta.env.DEV 为 false，整段被 tree-shake 掉。
if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) {
  const { installTauriMock } = await import('./dev/tauri-mock');
  installTauriMock();
}

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { ProgressWindow } from './features/progress-window/ProgressWindow.tsx'

// 票 08：进度小窗复用同一 SPA，通过哈希路由区分窗口角色
const hash = window.location.hash
const progressMatch = hash.match(/^#\/progress\/([a-zA-Z0-9-]+)/)

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {progressMatch ? <ProgressWindow taskId={progressMatch[1]} /> : <App />}
  </StrictMode>,
)
