import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { CheckCircle2, X, FolderOpen } from 'lucide-react';
import type { TaskInfo } from '../../store/useQuarkStore';
import { formatSize, cn } from '../../utils';
import { useAppearanceBootstrap } from '../../utils/appearance';

// 票 08：进度小窗——独立窗口，随任务进度实时更新；关闭小窗不影响下载
interface ViewEvent {
    taskId: string;
    view: 'progress' | 'completion';
}

// 惰性获取（同 TitleBar）：模块顶层调用会阻止浏览器内预览
let cachedWin: ReturnType<typeof getCurrentWindow> | null = null;
const win = () => (cachedWin ??= getCurrentWindow());

export const ProgressWindow: React.FC<{ taskId: string }> = ({ taskId }) => {
    useAppearanceBootstrap();
    const [task, setTask] = useState<TaskInfo | null>(null);
    const [view, setView] = useState<'progress' | 'completion'>('progress');
    const [autoClose, setAutoClose] = useState(
        () => localStorage.getItem(`progress_autoclose_${taskId}`) !== '0'
    );

    useEffect(() => {
        // 初始任务态
        invoke<TaskInfo[]>('tasks_snapshot')
            .then((tasks) => {
                const found = tasks.find((t) => t.id === taskId);
                if (found) {
                    setTask(found);
                    if (found.status === 'done') setView('completion');
                }
            })
            .catch(() => {});

        const unTasks = listen<{ type: string; task?: TaskInfo }>('task-event', (event) => {
            const ev = event.payload;
            if ((ev.type === 'upsert' || ev.type === 'progress') && ev.task?.id === taskId) {
                setTask(ev.task);
                if (ev.task.status === 'done') setView('completion');
            }
        });

        const unView = listen<ViewEvent>('progress://view', (event) => {
            if (event.payload.taskId === taskId) {
                setView(event.payload.view);
                // 「任务完成后自动关闭」本窗口覆盖项：关闭时不打断下载
                if (event.payload.view === 'completion' && localStorage.getItem(`progress_autoclose_${taskId}`) !== '0') {
                    setTimeout(() => win().close(), 2600);
                }
            }
        });

        return () => {
            unTasks.then((fn) => fn());
            unView.then((fn) => fn());
        };
    }, [taskId]);

    const close = () => win().close();

    const toggleAutoClose = () => {
        const next = !autoClose;
        setAutoClose(next);
        localStorage.setItem(`progress_autoclose_${taskId}`, next ? '1' : '0');
    };

    if (view === 'completion' && task) {
        return (
            <div className="flex h-screen flex-col justify-center rounded-xl bg-hud/95 p-4 text-hud-foreground shadow-2xl" style={{ fontFamily: '"Source Han Sans SC", "Noto Sans SC", "Microsoft YaHei", sans-serif' }}>
                <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-hud-foreground/15">
                        <CheckCircle2 size={22} />
                    </div>
                    <div className="min-w-0 flex-1">
                        <p className="text-xs font-bold">下载完成</p>
                        <p className="truncate text-[10px] text-hud-foreground/70" title={task.filename}>{task.filename}</p>
                    </div>
                    <button onClick={close} className="rounded-lg p-1 text-hud-foreground/60 hover:bg-hud-foreground/10 hover:text-hud-foreground">
                        <X size={14} />
                    </button>
                </div>
                <div className="mt-3 flex items-center gap-2">
                    <button
                        onClick={() => task.savedPath && invoke('open_folder', { path: task.savedPath })}
                        className="flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-hud-foreground/10 py-1.5 text-[10px] font-bold transition-colors hover:bg-hud-foreground/20"
                    >
                        <FolderOpen size={12} /> 打开所在文件夹
                    </button>
                    <button
                        onClick={toggleAutoClose}
                        className={cn(
                            'rounded-lg px-2 py-1.5 text-[9px] font-bold transition-colors',
                            autoClose ? 'bg-hud-foreground/10 text-hud-foreground/70' : 'bg-success/20 text-success'
                        )}
                        title={autoClose ? '本窗口即将自动关闭（仅此任务的设置）' : '本窗口保持打开'}
                    >
                        {autoClose ? '自动关闭开' : '自动关闭关'}
                    </button>
                </div>
            </div>
        );
    }

    const percent = task && task.size > 0 ? Math.min(100, Math.round((task.downloaded / task.size) * 100)) : 0;

    return (
        <div className="flex h-screen flex-col justify-center rounded-xl bg-hud/95 p-4 text-hud-foreground shadow-2xl" style={{ fontFamily: '"Source Han Sans SC", "Noto Sans SC", "Microsoft YaHei", sans-serif' }}>
            <div className="flex items-center justify-between gap-2">
                <p className="truncate text-xs font-bold" title={task?.filename ?? ''}>
                    {task?.filename ?? '准备中…'}
                </p>
                <button onClick={close} className="shrink-0 rounded-lg p-1 text-hud-foreground/60 hover:bg-hud-foreground/10 hover:text-hud-foreground">
                    <X size={14} />
                </button>
            </div>
            <div className="mt-2.5 flex items-center justify-between text-[10px] tabular-nums text-hud-foreground/70">
                <span>{task ? `${formatSize(task.downloaded)} / ${formatSize(task.size)}` : '—'}</span>
                <span className="font-bold text-hud-foreground">
                    {task && task.speed > 0 ? `${(task.speed / 1024 / 1024).toFixed(1)} MB/s` : ''}
                </span>
            </div>
            <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-hud-foreground/10">
                <div
                    className="h-full rounded-full bg-hud-foreground transition-all"
                    style={{ width: `${task?.status === 'merging' ? 100 : percent}%` }}
                />
            </div>
            <p className="mt-1.5 text-[9px] text-hud-foreground/60">
                {task?.status === 'saving'
                    ? '正在转存至网盘…'
                    : task?.status === 'merging'
                    ? '合并分片中…'
                    : `${percent}% · 关闭本窗口不会中断下载`}
            </p>
        </div>
    );
};
