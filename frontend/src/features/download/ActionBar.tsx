import React, { useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CheckSquare, Square, Loader2, Zap } from 'lucide-react';
import { useQuarkStore } from '../../store/useQuarkStore';
import { formatSize, cn, getErrorMessage } from '../../utils';
import { Button } from '../../components/ui/button';
import type { ShareFileNode } from '../../types/quark';

/**
 * 解析结果的操作条：全选 / 已选统计 / 开始任务。
 * 常驻显示，不再做成可折叠浮层（原折叠按钮无实际作用，已删除）。
 */
export const ActionBar: React.FC = () => {
    const {
        shareInfo,
        selectedFids,
        selectAllFiles,
        clearSelection,
        downloading,
        setDownloading,
        clearTasks,
        setBatchSummary,
        addLog,
        clearLogs,
        isLoggedIn,
        downloadConcurrency,
        downloadThreads,
        notify,
        setActiveTab,
    } = useQuarkStore();

    const selectedSize = useMemo(
        () => shareInfo.allFiles
            .filter((f: ShareFileNode) => selectedFids.has(f.fid))
            .reduce((acc: number, f: ShareFileNode) => acc + f.size, 0),
        [shareInfo.allFiles, selectedFids]
    );

    const allSelected = shareInfo.allFiles.length > 0 && selectedFids.size === shareInfo.allFiles.length;

    const toggleAll = () => {
        if (allSelected) clearSelection();
        else selectAllFiles(shareInfo.allFiles.map((f: ShareFileNode) => f.fid));
    };

    const startDownload = async () => {
        if (selectedFids.size === 0 || downloading) return;
        if (!isLoggedIn) {
            notify('请先登录账号', 'warn');
            return;
        }

        const files = shareInfo.allFiles
            .filter((f: ShareFileNode) => selectedFids.has(f.fid))
            .map((f: ShareFileNode) => ({
                fid: f.fid,
                shareFidToken: f.share_fid_token,
                fileName: f.file_name,
                size: f.size,
            }));

        clearTasks();
        clearLogs();
        setBatchSummary(null);

        // 同名文件「每次询问」策略：启动前检查并让用户决定
        let strategy: string | null = null;
        const settings = await invoke<{ duplicateStrategy: string }>('settings_get').catch(() => null);
        if (settings?.duplicateStrategy === 'ask') {
            const names = files.map((f) => f.fileName);
            const dup = await invoke<string[]>('check_duplicates', { names }).catch(() => [] as string[]);
            if (dup.length > 0) {
                const rename = window.confirm(
                    `有 ${dup.length} 个文件与已有文件重名（例如：${dup[0]}）。\n\n「确定」自动改名，「取消」跳过这些文件。`
                );
                strategy = rename ? 'rename' : 'skip';
            }
        }

        setDownloading(true);
        addLog(`已开始 ${files.length} 个任务，可在「下载任务」页查看进度`);

        try {
            await invoke('start_batch', {
                files,
                pwdId: shareInfo.pwdId,
                stoken: shareInfo.stoken,
                concurrency: downloadConcurrency,
                threads: downloadThreads,
                strategyOverride: strategy,
            });
            notify(`任务已提交，共 ${files.length} 个`, 'success');
            // 启动后自动切换到「下载任务」标签页，让用户直观看到下载进度
            setActiveTab('downloads');
        } catch (error: unknown) {
            setDownloading(false);
            const message = getErrorMessage(error);
            addLog(`启动失败: ${message}`, 'error');
            notify(`启动失败: ${message}`, 'error');
        }
    };

    return (
        <div className="flex flex-wrap items-center gap-4 rounded-2xl border border-border bg-card px-5 py-3 shadow-card">
            <button
                onClick={toggleAll}
                className={cn(
                    'flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm font-bold transition-colors',
                    allSelected ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                )}
            >
                {allSelected ? <CheckSquare size={16} /> : <Square size={16} />}
                全选
            </button>

            <span className="h-5 w-px bg-border" />

            <span className="flex items-center gap-2 text-sm text-muted-foreground">
                已选 <b className="text-foreground">{selectedFids.size}</b> 个
                <span className="rounded-md bg-muted px-2 py-0.5 text-xs font-bold text-foreground">
                    {formatSize(selectedSize)}
                </span>
            </span>

            <span className="hidden text-xs text-muted-foreground sm:inline">
                分片 {downloadThreads} · 并行 {downloadConcurrency}
            </span>

            <Button
                onClick={startDownload}
                disabled={selectedFids.size === 0 || downloading}
                className="ml-auto px-6 font-bold"
            >
                {downloading ? <Loader2 size={16} className="animate-spin" /> : <Zap size={16} />}
                开始任务
            </Button>
        </div>
    );
};
