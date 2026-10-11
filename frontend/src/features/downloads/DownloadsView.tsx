import React, { useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    Download, CheckCircle2, XCircle, X, RotateCcw, FolderOpen, File, Loader2,
    ListChecks, Ban,
} from 'lucide-react';
import { useQuarkStore, type TaskInfo, type TaskStatus } from '../../store/useQuarkStore';
import { formatSize, cn, getErrorMessage } from '../../utils';
import { Button } from '../../components/ui/button';

/**
 * 下载任务：与「解析提取」并列的独立页面（不再用悬浮岛承载进度）。
 * 状态与进度全部来自 Rust 侧推送的 task-event。
 */

const ACTIVE: TaskStatus[] = ['queued', 'saving', 'downloading', 'merging'];

const STATUS_META: Record<TaskStatus, { label: string; className: string; icon: React.ElementType }> = {
    queued: { label: '排队中', className: 'text-muted-foreground', icon: ListChecks },
    saving: { label: '转存中', className: 'text-warning', icon: Loader2 },
    downloading: { label: '下载中', className: 'text-primary', icon: Download },
    merging: { label: '合并中', className: 'text-primary', icon: Loader2 },
    done: { label: '已完成', className: 'text-success', icon: CheckCircle2 },
    failed: { label: '失败', className: 'text-danger', icon: XCircle },
    cancelled: { label: '已取消', className: 'text-muted-foreground', icon: Ban },
    skipped: { label: '已跳过', className: 'text-muted-foreground', icon: Ban },
};

/** R5.1：按已下载量与速度估算剩余时间 */
function formatEta(task: TaskInfo): string {
    if (task.status !== 'downloading' || task.speed <= 0 || task.size <= task.downloaded) return '';
    const seconds = Math.round((task.size - task.downloaded) / task.speed);
    if (!Number.isFinite(seconds) || seconds <= 0) return '';
    if (seconds < 60) return `剩 ${seconds} 秒`;
    if (seconds < 3600) return `剩 ${Math.round(seconds / 60)} 分`;
    return `剩 ${(seconds / 3600).toFixed(1)} 小时`;
}

const ProgressBar: React.FC<{ task: TaskInfo }> = ({ task }) => {
    const percent = task.size > 0 ? Math.min(100, Math.round((task.downloaded / task.size) * 100)) : 0;
    const barClass =
        task.status === 'done' ? 'bg-success'
        : task.status === 'failed' ? 'bg-danger'
        : task.status === 'saving' ? 'bg-warning/80'
        : task.status === 'cancelled' || task.status === 'skipped' ? 'bg-muted-foreground/40'
        : task.status === 'queued' ? 'bg-muted-foreground/40'
        : 'bg-primary';
    return (
        <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted">
            <div
                className={cn('h-full rounded-full transition-all', barClass)}
                style={{ width: `${task.status === 'done' ? 100 : percent}%` }}
            />
        </div>
    );
};

export const DownloadsView: React.FC = () => {
    const {
        tasks, batchSummary, downloading, clearTasks, setBatchSummary, notify, setTasks,
    } = useQuarkStore();

    // 挂载时主动拉取一次全量快照，保证切页时任务列表 100% 刷新就位
    React.useEffect(() => {
        invoke<TaskInfo[]>('tasks_snapshot')
            .then((snapshot) => {
                if (snapshot && snapshot.length > 0) {
                    setTasks(snapshot);
                }
            })
            .catch(() => {});
    }, [setTasks]);

    const stats = useMemo(() => ({
        active: tasks.filter((t) => ACTIVE.includes(t.status)).length,
        done: tasks.filter((t) => t.status === 'done').length,
        failed: tasks.filter((t) => t.status === 'failed').length,
    }), [tasks]);

    const cancelTask = async (id: string) => {
        await invoke('task_cancel', { id }).catch((e) => notify(`取消失败: ${getErrorMessage(e)}`, 'error'));
    };

    const cancelAll = async () => {
        await invoke('task_cancel_all').catch((e) => notify(`取消失败: ${getErrorMessage(e)}`, 'error'));
    };

    const retryTask = async (id: string) => {
        await invoke('task_retry', { id }).catch((e) => notify(`重试失败: ${getErrorMessage(e)}`, 'error'));
    };

    const openFolder = async (t: TaskInfo) => {
        if (!t.savedPath) return;
        await invoke('open_folder', { path: t.savedPath }).catch((e) => notify(`打开失败: ${getErrorMessage(e)}`, 'error'));
    };

    const openFile = async (t: TaskInfo) => {
        if (!t.savedPath) return;
        await invoke('open_file', { path: t.savedPath }).catch((e) => notify(`打开失败: ${getErrorMessage(e)}`, 'error'));
    };

    if (tasks.length === 0) {
        return (
            <div className="flex flex-1 flex-col items-center justify-center gap-4 text-center">
                <div className="flex h-16 w-16 items-center justify-center rounded-3xl bg-muted text-muted-foreground">
                    <Download size={30} strokeWidth={1.5} />
                </div>
                <div className="space-y-1">
                    <h3 className="text-lg font-bold text-foreground">还没有下载任务</h3>
                    <p className="text-sm text-muted-foreground">到「解析提取」选好文件后点「开始任务」，进度会出现在这里</p>
                </div>
            </div>
        );
    }

    return (
        <div className="flex flex-1 flex-col gap-4 min-h-0">
            {/* 汇总条 */}
            <div className="flex flex-wrap items-center gap-3 rounded-2xl border border-border bg-card px-5 py-4 shadow-card">
                <div className="flex items-center gap-2 text-sm font-bold text-foreground">
                    <Download size={16} className="text-primary" />
                    共 {tasks.length} 个任务
                </div>
                <span className="h-4 w-px bg-border" />
                <span className="text-sm text-muted-foreground">进行中 <b className="text-primary">{stats.active}</b></span>
                <span className="text-sm text-muted-foreground">完成 <b className="text-success">{stats.done}</b></span>
                <span className="text-sm text-muted-foreground">失败 <b className="text-danger">{stats.failed}</b></span>

                <div className="ml-auto flex items-center gap-2">
                    {stats.active > 0 && (
                        <Button variant="outline" size="sm" onClick={cancelAll} className="text-danger">
                            <X size={14} /> 全部取消
                        </Button>
                    )}
                    {!downloading && (
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={() => { clearTasks(); setBatchSummary(null); }}
                        >
                            清空列表
                        </Button>
                    )}
                </div>
            </div>

            {/* 失败汇总提示 */}
            {batchSummary && batchSummary.failed > 0 && (
                <div className="flex items-start gap-3 rounded-2xl border border-warning/30 bg-warning-soft px-4 py-3">
                    <XCircle size={18} className="mt-0.5 shrink-0 text-warning-on-soft" />
                    <p className="text-sm font-medium text-warning-on-soft">
                        本轮有 {batchSummary.failed} 个任务失败（共 {batchSummary.total} 个），可在下方任务上点「重试」。
                    </p>
                </div>
            )}

            {/* 任务列表 */}
            <div className="flex-1 min-h-0 overflow-y-auto rounded-2xl border border-border bg-card shadow-card">
                <div className="divide-y divide-border">
                    {tasks.map((t) => {
                        const meta = STATUS_META[t.status];
                        const Icon = meta.icon;
                        const isActive = ACTIVE.includes(t.status);
                        const percent = t.size > 0 ? Math.min(100, Math.round((t.downloaded / t.size) * 100)) : 0;
                        return (
                            <div key={t.id} className="px-5 py-4">
                                <div className="flex items-start gap-3">
                                    <Icon
                                        size={18}
                                        className={cn('mt-0.5 shrink-0', meta.className, t.status === 'merging' && 'animate-spin')}
                                    />
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-start justify-between gap-3">
                                            <p className="truncate text-sm font-semibold text-foreground" title={t.filename}>
                                                {t.filename}
                                            </p>
                                            <span className={cn('shrink-0 text-xs font-bold', meta.className)}>{meta.label}</span>
                                        </div>

                                        <ProgressBar task={t} />

                                        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                                            {t.status === 'saving' && (
                                                <span className="text-warning">正在转存至网盘（准备直链），请稍候…</span>
                                            )}
                                            {t.status === 'downloading' && (
                                                <>
                                                    <span className="tabular-nums">
                                                        {formatSize(t.downloaded)} / {formatSize(t.size)}（{percent}%）
                                                    </span>
                                                    {t.speed > 0 && (
                                                        <span className="tabular-nums text-foreground">
                                                            {(t.speed / 1024 / 1024).toFixed(1)} MB/s
                                                        </span>
                                                    )}
                                                    {formatEta(t) && <span className="tabular-nums">{formatEta(t)}</span>}
                                                </>
                                            )}
                                            {t.status === 'merging' && <span>正在合并分片…</span>}
                                            {t.status === 'done' && <span className="tabular-nums">{formatSize(t.size)}</span>}
                                            {(t.status === 'failed' || t.status === 'skipped') && t.error && (
                                                <span className="text-danger">{t.error}</span>
                                            )}
                                        </div>

                                        {/* 行内操作 */}
                                        <div className="mt-2 flex items-center gap-2">
                                            {isActive && (
                                                <button
                                                    onClick={() => cancelTask(t.id)}
                                                    className="rounded-lg border border-border px-2.5 py-1 text-xs font-bold text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                                                >
                                                    <X size={12} className="mr-1 inline" />取消
                                                </button>
                                            )}
                                            {t.status === 'failed' && (
                                                <button
                                                    onClick={() => retryTask(t.id)}
                                                    className="rounded-lg border border-primary/30 px-2.5 py-1 text-xs font-bold text-primary transition-colors hover:bg-primary/10"
                                                >
                                                    <RotateCcw size={12} className="mr-1 inline" />重试
                                                </button>
                                            )}
                                            {t.status === 'done' && t.savedPath && (
                                                <>
                                                    <button
                                                        onClick={() => openFile(t)}
                                                        className="rounded-lg border border-border px-2.5 py-1 text-xs font-bold text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                                                    >
                                                        <File size={12} className="mr-1 inline" />打开文件
                                                    </button>
                                                    <button
                                                        onClick={() => openFolder(t)}
                                                        className="rounded-lg border border-primary/30 px-2.5 py-1 text-xs font-bold text-primary transition-colors hover:bg-primary/10"
                                                    >
                                                        <FolderOpen size={12} className="mr-1 inline" />打开所在文件夹
                                                    </button>
                                                </>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
};
