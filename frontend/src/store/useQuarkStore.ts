import { create } from 'zustand';
import type { ShareFileNode } from '../types/quark';

export interface LogEntry {
    message: string;
    type: 'info' | 'success' | 'error' | 'warn';
    timestamp: number;
}

// 票 05：任务中心事件模型——任务状态与进度由 Rust 侧推送
export type TaskStatus = 'queued' | 'saving' | 'downloading' | 'merging' | 'done' | 'failed' | 'cancelled' | 'skipped';

export interface TaskInfo {
    id: string;
    filename: string;
    size: number;
    status: TaskStatus;
    error?: string;
    downloaded: number;
    speed: number;
    savedPath?: string;
}

export interface BatchSummary {
    done: number;
    failed: number;
    cancelled: number;
    total: number;
}

interface ShareParseState {
    pwdId: string;
    stoken: string;
    files: ShareFileNode[];
    allFiles: ShareFileNode[];
    status: string;
    statusType: 'info' | 'success' | 'error' | 'warn';
    shareUrl: string;
}

interface QuarkState {
    nickname: string;
    sessionExpired: boolean;
    isAuthOpen: boolean;
    toast: {
        isLoggingOut?: boolean;
        id: number;
        message: string;
        type: 'info' | 'success' | 'error' | 'warn' | '';
    };
    isLoggedIn: boolean;
    capacity: {
        used: number;
        total: number;
        loading: boolean;
    };
    shareInfo: ShareParseState;
    logs: LogEntry[];
    downloading: boolean;
    tasks: TaskInfo[];
    batchSummary: BatchSummary | null;
    selectedFids: Set<string>;
    downloadConcurrency: number;
    downloadThreads: number;
    activeTab: string;

    setActiveTab: (tab: string) => void;
    setLoginState: (isLoggedIn: boolean, nickname: string) => void;
    setSessionExpired: (expired: boolean) => void;
    setIsAuthOpen: (open: boolean) => void;
    setCapacity: (used: number, total: number) => void;
    setCapacityLoading: (loading: boolean) => void;
    setShareInfo: (info: Partial<ShareParseState>) => void;
    addLog: (message: string, type?: LogEntry['type']) => void;
    clearLogs: () => void;
    setDownloadConcurrency: (value: number) => void;
    setDownloadThreads: (value: number) => void;
    setLoggingOut: (isLoggingOut: boolean) => void;
    notify: (message: string, type: 'info' | 'success' | 'error' | 'warn', durationMs?: number) => void;
    toggleFileSelection: (fid: string) => void;
    selectAllFiles: (fids: string[]) => void;
    clearSelection: () => void;
    setDownloading: (downloading: boolean) => void;
    setTasks: (tasks: TaskInfo[]) => void;
    upsertTask: (task: TaskInfo) => void;
    clearTasks: () => void;
    setBatchSummary: (summary: BatchSummary | null) => void;
    resetShareInfo: () => void;
    setShareUrl: (url: string) => void;
}

/** 读取并夹取本地存储的整数设置；非法值回落到默认值 */
function clampInt(raw: string | null, min: number, max: number, fallback: number): number {
    const n = Number(raw);
    if (!Number.isFinite(n) || n <= 0) return fallback;
    return Math.max(min, Math.min(max, Math.trunc(n)));
}

export const useQuarkStore = create<QuarkState>((set) => ({
    toast: {
        id: 0,
        message: '',
        type: '',
    },
    // 登录态由 Rust 侧系统凭据库托管（票 03）：前端只持有昵称与布尔位
    nickname: '',
    isLoggedIn: false,
    sessionExpired: false,
    isAuthOpen: false,
    capacity: {
        used: 0,
        total: 0,
        loading: false,
    },
    shareInfo: {
        pwdId: '',
        stoken: '',
        files: [],
        allFiles: [],
        status: '',
        statusType: 'info',
        shareUrl: '',
    },
    logs: [],
    downloading: false,
    tasks: [],
    batchSummary: null,
    selectedFids: new Set<string>(),
    // 从 localStorage 恢复时必须夹取：旧版本（上游默认 999）留下的值
    // 若不夹取，界面会显示 128 之类超限数字，而引擎静默按 64 执行——UI 说谎。
    downloadConcurrency: clampInt(localStorage.getItem('download_concurrency'), 1, 10, 3),
    downloadThreads: clampInt(localStorage.getItem('download_threads'), 1, 64, 12),
    activeTab: 'parser',

    setActiveTab: (activeTab: string) => set({ activeTab }),
    setLoginState: (isLoggedIn: boolean, nickname: string) =>
        set((state: QuarkState) => ({
            isLoggedIn,
            nickname,
            sessionExpired: isLoggedIn ? false : state.sessionExpired,
        })),
    setSessionExpired: (sessionExpired: boolean) => set({ sessionExpired }),
    setIsAuthOpen: (isAuthOpen: boolean) => set({ isAuthOpen }),
    setCapacity: (used: number, total: number) => set({ capacity: { used, total, loading: false } }),
    setCapacityLoading: (loading: boolean) => set((state: QuarkState) => ({ capacity: { ...state.capacity, loading } })),
    setShareInfo: (info: Partial<ShareParseState>) => {
        if (info.files) {
            const flatten = (nodes: ShareFileNode[]): ShareFileNode[] => {
                const res: ShareFileNode[] = [];
                for (const n of nodes) {
                    if (!n.isDir) res.push(n);
                    if (n.children) res.push(...flatten(n.children));
                }
                return res;
            };
            info.allFiles = flatten(info.files);
        }

        set((state: QuarkState) => ({ shareInfo: { ...state.shareInfo, ...info } }));
    },
    addLog: (message: string, type: LogEntry['type'] = 'info') => {
        const newLog: LogEntry = {
            message,
            type,
            timestamp: Date.now(),
        };
        set((state: QuarkState) => ({ logs: [newLog, ...state.logs] }));
    },
    clearLogs: () => set({ logs: [] }),
    setDownloadConcurrency: (value: number) => {
        const normalized = Math.max(1, Math.min(10, value));
        localStorage.setItem('download_concurrency', normalized.toString());

        set({ downloadConcurrency: normalized });
    },
    setDownloadThreads: (value: number) => {
        const normalized = Math.max(1, Math.min(64, value));
        localStorage.setItem('download_threads', normalized.toString());

        set({ downloadThreads: normalized });
    },
    notify: (message: string, type: 'info' | 'success' | 'error' | 'warn', durationMs?: number) => {
        const toastId = Date.now() + Math.random();
        const timeout = durationMs ?? 3000;
        set({ toast: { id: toastId, message, type } });

        setTimeout(() => {
            set((state: QuarkState) => (
                state.toast.id === toastId ? { toast: { id: 0, message: '', type: '' } } : {}
            ));
        }, timeout);
    },
    setLoggingOut: (isLoggingOut: boolean) => set((state: QuarkState) => ({ toast: { ...state.toast, isLoggingOut } })),
    toggleFileSelection: (fid: string) => set((state: QuarkState) => {
        const newSet = new Set(state.selectedFids);
        if (newSet.has(fid)) newSet.delete(fid);
        else newSet.add(fid);
        return { selectedFids: newSet };
    }),
    selectAllFiles: (fids: string[]) => set({ selectedFids: new Set(fids) }),
    clearSelection: () => set({ selectedFids: new Set<string>() }),
    setDownloading: (downloading: boolean) => set({ downloading }),
    setTasks: (tasks: TaskInfo[]) => set({ tasks }),
    upsertTask: (task: TaskInfo) => set((state: QuarkState) => {
        const idx = state.tasks.findIndex((t) => t.id === task.id);
        const next = idx >= 0
            ? state.tasks.map((t) => (t.id === task.id ? task : t))
            : [...state.tasks, task];
        return { tasks: next };
    }),
    clearTasks: () => set({ tasks: [], batchSummary: null }),
    setBatchSummary: (summary: BatchSummary | null) => set({ batchSummary: summary }),
    resetShareInfo: () => set((state: QuarkState) => ({
        shareInfo: {
            pwdId: '',
            stoken: '',
            files: [],
            allFiles: [],
            status: '',
            statusType: 'info',
            shareUrl: state.shareInfo.shareUrl,
        },
        selectedFids: new Set<string>(),
        logs: [],
    })),
    setShareUrl: (url: string) => set((state: QuarkState) => ({
        shareInfo: { ...state.shareInfo, shareUrl: url }
    })),
}));

// 开发预览：把 store 挂到 window 便于在浏览器里注入状态校对界面（产线构建剔除）
if (import.meta.env.DEV) {
    (window as unknown as Record<string, unknown>).__quarkStore = useQuarkStore;
}
