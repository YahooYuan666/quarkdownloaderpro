import React from 'react';
import { motion } from 'framer-motion';
import {
    Zap,
    Settings,
    HelpCircle,
    LogOut,
    User,
    HardDrive,
    RefreshCw,
    ChevronRight,
    Download,
} from 'lucide-react';
import { cn } from '../utils';
import { useQuarkStore } from '../store/useQuarkStore';
import { Button } from './ui/button';
import { CapacityStats } from '../features/capacity/CapacityStats';

interface SidebarProps {
    onLogout: () => void;
    onRefreshCapacity: () => void;
    onLogin: () => void;
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
    onLogout,
    onRefreshCapacity,
    onLogin,
    activeTab,
    setActiveTab
}) => {
    const { isLoggedIn, nickname, toast } = useQuarkStore();

    const menuItems = [
        { id: 'parser', icon: Zap, label: '解析提取' },
        { id: 'downloads', icon: Download, label: '下载任务' },
        { id: 'settings', icon: Settings, label: '软件设置' },
        { id: 'help', icon: HelpCircle, label: '使用说明' },
    ];

    return (
        <div className="relative m-6 w-52 bg-card border border-border shadow-card rounded-[2.5rem] z-50 flex flex-col p-5 overflow-hidden shrink-0 h-[calc(100%-3rem)]">
            <div className="flex items-center gap-2 mb-10 px-1">
                <div className="w-10 h-10 gradient-bg rounded-2xl flex items-center justify-center text-primary-foreground shadow-lg shadow-card">
                    <HardDrive size={22} strokeWidth={2.5} />
                </div>
                <div className="flex flex-col">
                    <span className="font-display text-base font-medium tracking-tight text-foreground leading-none">
                        Quark
                    </span>
                    <span className="font-display text-xs font-medium tracking-wide text-muted-foreground">
                        Downloader Pro
                    </span>
                </div>
            </div>

            <nav className="flex-1 space-y-2">
                {menuItems.map((item) => (
                    <button
                        key={item.id}
                        onClick={() => setActiveTab(item.id)}
                        className={cn(
                            "w-full flex items-center gap-2.5 px-3.5 py-3 rounded-2xl transition-all duration-200 group relative",
                            activeTab === item.id
                                ? "bg-primary text-primary-foreground shadow-sm font-semibold"
                                : "text-muted-foreground hover:bg-muted hover:text-foreground"
                        )}
                    >
                        <item.icon size={20} strokeWidth={activeTab === item.id ? 2.5 : 2} />
                        <span className="font-semibold text-sm">{item.label}</span>
                        {activeTab === item.id && (
                            <motion.div
                                layoutId="active-pill"
                                className="absolute right-3"
                            >
                                <ChevronRight size={14} className="opacity-75" />
                            </motion.div>
                        )}
                    </button>
                ))}
            </nav>

            <div className="mt-auto space-y-4">
                {/* 转移过来的并发设置 */}
                {activeTab === 'parser' && (
                    <div className="space-y-2">
                        <div className="p-3.5 bg-muted/50 rounded-2xl border border-border">
                            <div className="flex items-center justify-between mb-2">
                                <span className="text-[10px] font-bold text-muted-foreground uppercase">单文件分片数</span>
                                <input
                                    type="number"
                                    min="1" max="64"
                                    value={useQuarkStore.getState().downloadThreads}
                                    onChange={(e) => useQuarkStore.getState().setDownloadThreads(Number(e.target.value))}
                                    className="w-14 text-right text-xs font-black text-primary bg-transparent outline-none border-b border-primary/30 focus:border-primary [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                                />
                            </div>
                            <input
                                type="range"
                                min="1" max="64"
                                value={Math.min(useQuarkStore.getState().downloadThreads, 64)}
                                onChange={(e) => useQuarkStore.getState().setDownloadThreads(Number(e.target.value))}
                                className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer accent-primary"
                            />
                        </div>
                        <div className="p-3.5 bg-muted/50 rounded-2xl border border-border">
                            <div className="flex items-center justify-between mb-2">
                                <span className="text-[10px] font-bold text-muted-foreground uppercase">同时下载</span>
                                <span className="text-xs font-black text-primary">{useQuarkStore.getState().downloadConcurrency}</span>
                            </div>
                            <input
                                type="range"
                                min="1" max="10"
                                value={useQuarkStore.getState().downloadConcurrency}
                                onChange={(e) => useQuarkStore.getState().setDownloadConcurrency(Number(e.target.value))}
                                className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer accent-primary"
                            />
                        </div>
                    </div>
                )}

                {isLoggedIn ? (
                    <div className="space-y-4">
                        <div className="flex items-center gap-2 rounded-2xl border border-success/25/60 bg-success-soft/60 px-3 py-2.5">
                            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-success text-[10px] font-black text-primary-foreground">
                                {(nickname || '夸').slice(0, 1)}
                            </span>
                            <div className="min-w-0">
                                <p className="text-[9px] font-bold uppercase tracking-widest text-success">已登录</p>
                                <p className="truncate text-xs font-bold text-foreground">{nickname || '夸克账号'}</p>
                            </div>
                        </div>
                        <CapacityStats onRefresh={onRefreshCapacity} />
                        <motion.button
                            disabled={toast.isLoggingOut}
                            whileHover={{ scale: 1.02 }}
                            whileTap={{ scale: 0.98 }}
                            onClick={onLogout}
                            className={cn(
                                "w-full flex items-center justify-center gap-2 py-2.5 text-[10px] font-bold rounded-xl transition-all duration-300 border shadow-sm",
                                toast.isLoggingOut
                                    ? "bg-muted text-muted-foreground border-border cursor-not-allowed"
                                    : "text-danger border-danger/25/50 hover:bg-danger hover:text-primary-foreground hover:border-danger hover:shadow-md hover:shadow-card"
                            )}
                        >
                            {toast.isLoggingOut ? <RefreshCw size={14} className="animate-spin" /> : <LogOut size={14} strokeWidth={2.5} />}
                            {toast.isLoggingOut ? '正在提出...' : '退出登录'}
                        </motion.button>
                    </div>
                ) : (
                    <Button
                        onClick={onLogin}
                                                className="w-full rounded-2xl py-6 font-bold flex items-center gap-2 group"
                    >
                        <User size={18} className="group-hover:animate-pulse" />
                        立即登录
                    </Button>
                )}
            </div>
        </div>
    );
};