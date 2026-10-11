import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderInput, RotateCcw, ExternalLink, Sun, Moon, MonitorCog } from 'lucide-react';
import { useQuarkStore } from '../../store/useQuarkStore';
import { cn } from '../../utils';
import { applyAppearance, type Appearance } from '../../utils/appearance';

// 票 06：设置页——下载目录、记住上次目录、同名策略、线程/并发
interface Settings {
    downloadDir: string | null;
    lastDir: string | null;
    rememberLastDir: boolean;
    duplicateStrategy: string;
    appearance: string;
    progressWindowOnCompletion: boolean;
}

const Row: React.FC<{ title: string; desc?: string; children: React.ReactNode }> = ({ title, desc, children }) => (
    <div className="flex items-center justify-between gap-6 py-4">
        <div className="min-w-0">
            <p className="text-sm font-bold text-foreground">{title}</p>
            {desc && <p className="mt-1 text-xs font-medium leading-relaxed text-muted-foreground">{desc}</p>}
        </div>
        <div className="shrink-0">{children}</div>
    </div>
);

const Toggle: React.FC<{ on: boolean; onChange: (v: boolean) => void }> = ({ on, onChange }) => (
    <button
        onClick={() => onChange(!on)}
        className={cn(
            'relative h-6 w-11 rounded-full transition-colors',
            on ? 'bg-primary' : 'bg-muted'
        )}
    >
        <span
            className={cn(
                'absolute top-0.5 h-5 w-5 rounded-full bg-card shadow transition-all',
                on ? 'left-[22px]' : 'left-0.5'
            )}
        />
    </button>
);

export const SettingsView: React.FC = () => {
    const [settings, setSettings] = useState<Settings | null>(null);
    const {
        downloadThreads,
        setDownloadThreads,
        downloadConcurrency,
        setDownloadConcurrency,
        notify,
    } = useQuarkStore();

    useEffect(() => {
        invoke<Settings>('settings_get').then(setSettings).catch((e) => notify(`读取设置失败: ${e}`, 'error'));
    }, [notify]);

    const patch = async (p: Record<string, unknown>) => {
        const next = await invoke<Settings>('settings_set', { patch: p }).catch((e) => {
            notify(`保存设置失败: ${e}`, 'error');
            return null;
        });
        if (next) setSettings(next);
    };

    const pickDir = async () => {
        const picked = await invoke<string | null>('pick_folder').catch((e) => {
            notify(`打开目录选择器失败: ${e}`, 'error');
            return null;
        });
        if (picked) {
            await patch({ downloadDir: picked });
            notify('下载目录已更新', 'success');
        }
    };

    const openDir = async () => {
        const dir = settings?.downloadDir || settings?.lastDir;
        if (dir) await invoke('open_folder', { path: dir });
    };

    if (!settings) return null;

    return (
        <div className="w-full max-w-xl space-y-6 p-6">
            <h2 className="text-xl font-black text-foreground">软件设置</h2>

            <div className="divide-y divide-border rounded-2xl border border-border bg-card/70 p-4 backdrop-blur-sm">
                <Row title="下载目录" desc={settings.downloadDir || settings.lastDir || '使用系统默认下载目录'}>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={pickDir}
                            className="flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-xs font-bold text-muted-foreground transition-all hover:bg-muted"
                        >
                            <FolderInput size={13} /> 选择目录
                        </button>
                        {(settings.downloadDir || settings.lastDir) && (
                            <button
                                onClick={openDir}
                                className="flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-xs font-bold text-muted-foreground transition-all hover:bg-muted"
                            >
                                <ExternalLink size={13} /> 打开
                            </button>
                        )}
                    </div>
                </Row>

                <Row title="记住上次目录" desc="开启后，最近一次选择的目录在下一次启动时仍然生效">
                    <Toggle on={settings.rememberLastDir} onChange={(v) => patch({ rememberLastDir: v })} />
                </Row>

                <Row title="同名文件" desc="下载时目标位置已存在同名文件的处理方式">
                    <select
                        value={settings.duplicateStrategy}
                        onChange={(e) => patch({ duplicateStrategy: e.target.value })}
                        className="rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-bold text-muted-foreground outline-none focus:border-primary"
                    >
                        <option value="rename">自动改名 (1)(2)</option>
                        <option value="skip">跳过</option>
                        <option value="ask">每次询问</option>
                    </select>
                </Row>

                <Row title="外观" desc="深色模式可跟随系统，也可手动固定">
                    <div className="flex overflow-hidden rounded-lg border border-border">
                        {([
                            { v: 'system', label: '跟随系统', icon: MonitorCog },
                            { v: 'light', label: '浅色', icon: Sun },
                            { v: 'dark', label: '深色', icon: Moon },
                        ] as const).map(({ v, label, icon: Icon }) => (
                            <button
                                key={v}
                                onClick={() => {
                                    patch({ appearance: v });
                                    applyAppearance(v as Appearance);
                                }}
                                className={cn(
                                    'flex items-center gap-1 px-3 py-1.5 text-xs font-bold transition-colors',
                                    settings.appearance === v
                                        ? 'bg-primary text-primary-foreground'
                                        : 'bg-card text-muted-foreground hover:bg-muted'
                                )}
                            >
                                <Icon size={12} /> {label}
                            </button>
                        ))}
                    </div>
                </Row>

                <Row title="下载完成时显示完成窗" desc="单个任务下载结束后弹出完成提示（仍可在小窗内单独关闭）">
                    <Toggle
                        on={settings.progressWindowOnCompletion}
                        onChange={(v) => patch({ progressWindowOnCompletion: v })}
                    />
                </Row>

                <Row title="单文件分片数" desc="大文件同时下载的连接数（1–64）">
                    <input
                        type="number"
                        min={1}
                        max={64}
                        value={downloadThreads}
                        onChange={(e) => setDownloadThreads(Number(e.target.value))}
                        className="w-20 rounded-lg border border-border px-3 py-1.5 text-right text-xs font-bold text-muted-foreground outline-none focus:border-primary"
                    />
                </Row>

                <Row title="同时下载文件数" desc="批量下载时并行的文件个数（1–10）">
                    <input
                        type="number"
                        min={1}
                        max={10}
                        value={downloadConcurrency}
                        onChange={(e) => setDownloadConcurrency(Number(e.target.value))}
                        className="w-20 rounded-lg border border-border px-3 py-1.5 text-right text-xs font-bold text-muted-foreground outline-none focus:border-primary"
                    />
                </Row>
            </div>

            <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                <RotateCcw size={11} /> 设置保存在本机，不上传任何数据
            </p>
        </div>
    );
};
