import React, { type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Sidebar } from '../components/Sidebar';
import { TitleBar } from '../components/TitleBar';
import { useQuarkStore } from '../store/useQuarkStore';
import { quarkApi } from '../services/quarkApi';
import { AuthModal } from '../features/auth/AuthModal';
import { SessionExpiredBanner } from '../features/auth/SessionExpiredBanner';
import { motion, AnimatePresence } from 'framer-motion';
import { Zap, CheckCircle2, AlertCircle, Info, AlertTriangle } from 'lucide-react';
import { getErrorMessage } from '../utils';
const ToastIcon: React.FC<{ type: string }> = ({ type }) => {
    switch (type) {
        case 'success': return <CheckCircle2 size={18} className="text-success" />;
        case 'error': return <AlertCircle size={18} className="text-danger" />;
        case 'warn': return <AlertTriangle size={18} className="text-warning" />;
        case 'info': return <Info size={18} className="text-primary" />;
        default: return <Zap size={18} className="text-primary" />;
    }
};

interface MainLayoutProps {
    children: ReactNode;
    activeTab: string;
    setActiveTab: (tab: string) => void;
    setIsAuthOpen: (open: boolean) => void;
    isAuthOpen: boolean;
}

export const MainLayout: React.FC<MainLayoutProps> = ({
    children,
    activeTab,
    setActiveTab,
    isAuthOpen,
    setIsAuthOpen
}) => {
    const {
        isLoggedIn,
        setLoginState,
        addLog,
        setCapacity,
        setCapacityLoading,
        toast,
        notify,
        setLoggingOut,
    } = useQuarkStore();


    const fetchCapacity = async () => {
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
    };

    const handleLogout = async () => {
        setLoggingOut(true);
        let logoutMessage = '已安全退出登录';
        let logoutType: 'success' | 'warn' = 'success';

        try {
            const result = await quarkApi.logout();
            if (result.logged_out) {
                logoutType = result.logout_request_error ? 'warn' : 'success';
                logoutMessage = result.message || '已安全退出夸克账号';
            } else {
                logoutType = 'warn';
                const nickname = result.verify?.nickname ? `（当前账号：${result.verify.nickname}）` : '';
                logoutMessage = result.message || `已清理本地登录状态，远端会话可能仍有效${nickname}`;
            }

            if (result.verify_error) {
                logoutType = 'warn';
            }
        } catch (error: unknown) {
            logoutType = 'warn';
            logoutMessage = `远端退出状态校验失败，已清理本地登录状态（${getErrorMessage(error)}）`;
        }

        await invoke('cookie_clear').catch(() => {});
        setLoginState(false, '');
        setCapacity(0, 0);
        addLog('已退出登录', 'info');
        notify(logoutMessage, logoutType);
        setLoggingOut(false);
    };

    const hasToast = Boolean(toast.message);
    const hasIslandContent = hasToast;

    return (
        <div className="h-screen flex flex-col bg-background text-foreground font-sans selection:bg-primary/20 selection:text-primary-foreground overflow-hidden relative">
            {/* 自定义标题栏 */}
            <TitleBar />

            {/* 主内容区 */}
            <div className="flex-1 flex min-h-0 relative">
                {/* 提示气泡：只承载瞬时提示，下载进度已移至「下载任务」页 */}
                <div className="fixed top-12 left-1/2 -translate-x-1/2 z-[100] px-4 pointer-events-none">
                    <AnimatePresence>
                        {hasIslandContent && (
                            <motion.div
                                key="toast"
                                initial={{ y: -24, opacity: 0, scale: 0.94 }}
                                animate={{ y: 0, opacity: 1, scale: 1 }}
                                exit={{ y: -24, opacity: 0, scale: 0.94 }}
                                transition={{ type: 'spring', damping: 24, stiffness: 280 }}
                                className="mx-auto flex w-fit max-w-[calc(100vw-2rem)] items-center gap-3 rounded-2xl border border-hud-foreground/10 bg-hud/95 px-5 py-3 shadow-pop backdrop-blur-xl"
                            >
                                <ToastIcon type={toast.type} />
                                <span className="text-sm font-semibold text-hud-foreground">{toast.message}</span>
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>

            <Sidebar
                onLogout={handleLogout}
                onRefreshCapacity={fetchCapacity}
                onLogin={() => setIsAuthOpen(true)}
                activeTab={activeTab}
                setActiveTab={setActiveTab}
            />

            <div className="relative flex h-full min-h-0 flex-1 flex-col overflow-hidden pl-2 pr-6 py-6">
                <SessionExpiredBanner />
                <main className="flex-1 w-full flex flex-col min-h-0">
                    {children}
                </main>
            </div>

            <AuthModal isOpen={isAuthOpen} onClose={() => setIsAuthOpen(false)} onLogout={handleLogout} />
            </div>
        </div>
    );
};
