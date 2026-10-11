import React, { useCallback, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ShieldCheck, HardDrive } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useAppearanceBootstrap } from './utils/appearance';
import { useQuarkStore } from './store/useQuarkStore';
import { quarkApi } from './services/quarkApi';
import { ShareParser } from './features/share-parse/ShareParser';
import { HelpView } from './features/help/HelpView';
import { SettingsView } from './features/settings/SettingsView';
import { DownloadsView } from './features/downloads/DownloadsView';
import { FileTree } from './features/file-tree/FileTree';
import { Badge } from './components/ui/badge';
import { MainLayout } from './components/MainLayout';
import type { TaskInfo, BatchSummary } from './store/useQuarkStore';

function App() {
  const {
    isLoggedIn,
    setCapacity,
    setCapacityLoading,
    setLoginState,
    shareInfo,
  } = useQuarkStore();

  const isAuthOpen = useQuarkStore((s) => s.isAuthOpen);
  const setIsAuthOpen = useQuarkStore((s) => s.setIsAuthOpen);
  const setSessionExpired = useQuarkStore((s) => s.setSessionExpired);
  const addLog = useQuarkStore((s) => s.addLog);
  const notify = useQuarkStore((s) => s.notify);
  const activeTab = useQuarkStore((s) => s.activeTab);
  const setActiveTab = useQuarkStore((s) => s.setActiveTab);
  useAppearanceBootstrap();

  // 票 07：Rust 侧检测到登录态失效 → 提示 + 展示重登指引
  useEffect(() => {
    const unlisten = listen('session://expired', (event) => {
      const payload = (event.payload as { reason?: string }) || {};
      setSessionExpired(true);
      setLoginState(false, '');
      addLog('登录状态已失效，请重新提取 Cookie', 'warn');
      notify(payload.reason || '登录状态已失效', 'warn', 5000);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setSessionExpired, setLoginState, addLog, notify]);

  // 票 05/08：监听 task-event 更新任务中心 store
  useEffect(() => {
    const { upsertTask, setTasks, setBatchSummary, setDownloading } = useQuarkStore.getState();
    let unlisten: (() => void) | null = null;

    (async () => {
      unlisten = await listen<{ type: string; task?: TaskInfo; tasks?: TaskInfo[]; summary?: BatchSummary }>('task-event', (event) => {
        const ev = event.payload;
        switch (ev.type) {
          case 'snapshot':
            if (ev.tasks) setTasks(ev.tasks);
            break;
          case 'upsert':
          case 'progress':
            if (ev.task) upsertTask(ev.task);
            break;
          case 'batch-done':
            if (ev.summary) setBatchSummary(ev.summary);
            setDownloading(false);
            break;
        }
      });
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 票 03：启动时从 Rust 侧恢复登录态（Cookie 在系统凭据库中，前端不接触明文）
  useEffect(() => {
    (async () => {
      try {
        // 凭据库不可用会导致"每次启动都要重新登录"，这里提前告诉用户
        const storage = await invoke<{ available: boolean }>('cookie_storage_ok');
        if (!storage.available) {
          notify('系统凭据库不可用，登录状态无法保存（每次启动需重新登录）', 'warn', 6000);
        }
        const state = await invoke<{ has: boolean }>('cookie_state');
        if (!state.has) return;
        const who = await invoke<{ logged_in: boolean; nickname: string }>('cookie_whoami');
        setLoginState(who.logged_in, who.nickname || '');
      } catch (e) {
        console.error('恢复登录态失败', e);
      }
    })();
  }, [setLoginState, notify]);

  const fetchCapacity = useCallback(async () => {
    if (!isLoggedIn) return;
    setCapacityLoading(true);
    try {
      const res = await quarkApi.getMemberInfo();
      if (res.status === 200 || res.code === 0) {
        const data = res.data || res;
        setCapacity(data.use_capacity || 0, data.total_capacity || 0);
      }
    } catch (e) {
      console.error('Failed to fetch capacity', e);
    }
  }, [isLoggedIn, setCapacity, setCapacityLoading]);

  React.useEffect(() => {
    if (!isLoggedIn) return;
    void fetchCapacity();
  }, [isLoggedIn, fetchCapacity]);

  return (
    <MainLayout
      activeTab={activeTab}
      setActiveTab={setActiveTab}
      isAuthOpen={isAuthOpen}
      setIsAuthOpen={setIsAuthOpen}
    >
      <AnimatePresence mode="wait">
        {activeTab === 'parser' && (
          <motion.div
            key="parser-tab"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.3 }}
            className="flex-1 flex flex-col gap-6 min-h-0"
          >
            {/* Hero Section */}
            <section className="text-left pt-2 shrink-0">
              <motion.div
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: 0.1 }}
              >
                <Badge variant="indigo" className="mb-2 py-1 px-3 rounded-full">
                  <ShieldCheck size={14} className="mr-2" />
                  Quark Engine v2.0 安全连接已就绪
                </Badge>
              </motion.div>
              <h1 className="text-3xl font-extrabold tracking-tight text-foreground mb-1">
                链接提取 <span className="gradient-text">从未如此简单</span>
              </h1>
              <p className="text-muted-foreground text-sm max-w-2xl leading-relaxed font-medium">
                粘贴您的分享链接，我们将为您自动化处理剩余的一切。
              </p>
            </section>

            {/* Parser Section (Now includes the Floating Action Card) */}
            <section className="shrink-0">
              <ShareParser />
            </section>

            {/* Dynamic Results Area */}
            <div className="flex-1 flex flex-col min-h-0">
              <AnimatePresence mode="popLayout" initial={false}>
                {shareInfo.files.length > 0 ? (
                  <motion.div
                    key="results-area"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="flex-1 min-h-0"
                  >
                    <FileTree />
                  </motion.div>
                ) : (
                  <motion.div
                    key="empty-state"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className="flex-1 flex flex-col items-center justify-center text-center space-y-4 bg-card border border-border shadow-card rounded-[2.5rem] border-dashed border-2 border-border"
                  >
                    <div className="w-16 h-16 bg-muted/50 rounded-3xl flex items-center justify-center text-muted-foreground">
                      <HardDrive size={32} strokeWidth={1.5} />
                    </div>
                    <div className="space-y-1">
                      <h3 className="text-foreground font-bold text-lg">等待任务投递</h3>
                      <p className="text-muted-foreground text-xs max-w-xs font-medium">
                        在上方粘贴链接，开启极速解析体验
                      </p>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </motion.div>
        )}

        {activeTab === 'downloads' && (
          <motion.div
            key="downloads-tab"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="flex-1 flex flex-col min-h-0"
          >
            <DownloadsView />
          </motion.div>
        )}

        {activeTab === 'settings' && (
          <motion.div
            key="settings-tab"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="flex-1 flex flex-col items-center justify-center"
          >
            <SettingsView />
          </motion.div>
        )}

        {activeTab === 'help' && (
          <motion.div
            key="help-tab"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="flex-1 flex flex-col min-h-0"
          >
            <HelpView />
          </motion.div>
        )}
      </AnimatePresence>
    </MainLayout>
  );
}

export default App;
