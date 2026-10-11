import React, { useState } from 'react';
import { AlertTriangle, RefreshCw, X } from 'lucide-react';
import { useQuarkStore } from '../../store/useQuarkStore';

// 票 07：会话失效横幅——大白话、可照做的重新登录指引
const STEPS = [
    '打开浏览器访问 pan.quark.cn，登录你的夸克账号',
    '登录后按 F12 打开开发者工具，点顶部「网络」（或 Network）标签',
    '按 F5 刷新页面，在列表里点第一条（名称通常是 list，类型是 document）',
    '在右侧「标头」→「请求标头」里找到 Cookie: 这一行，右键复制它的值',
    '回到本应用，点左侧「立即登录」→「手动输入」，粘贴进去保存',
];

export const SessionExpiredBanner: React.FC = () => {
    const { sessionExpired, setSessionExpired, setIsAuthOpen } = useQuarkStore();
    const [expanded, setExpanded] = useState(false);
    const [dismissed, setDismissed] = useState(false);

    if (!sessionExpired || dismissed) return null;

    return (
        <div className="mx-3 mb-2 rounded-2xl border border-warning/30 bg-warning-soft p-4 backdrop-blur-sm">
            <div className="flex items-start gap-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-warning/20 text-warning">
                    <AlertTriangle size={18} />
                </div>
                <div className="min-w-0 flex-1">
                    <p className="text-xs font-bold text-warning-on-soft">登录状态已失效，需要重新提取一次 Cookie</p>
                    <p className="mt-0.5 text-[11px] font-medium text-warning-on-soft">
                        按下面的步骤做一遍就能恢复（大约 1 分钟）：
                    </p>

                    {expanded && (
                        <ol className="mt-2 space-y-1.5">
                            {STEPS.map((s, i) => (
                                <li key={i} className="flex gap-2 text-[11px] font-medium leading-relaxed text-warning-on-soft">
                                    <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-warning/30 text-[9px] font-black text-warning-on-soft">
                                        {i + 1}
                                    </span>
                                    {s}
                                </li>
                            ))}
                        </ol>
                    )}

                    <div className="mt-2.5 flex items-center gap-2">
                        <button
                            onClick={() => setExpanded(!expanded)}
                            className="rounded-lg border border-warning/40 px-3 py-1.5 text-[11px] font-bold text-warning-on-soft transition-all hover:bg-warning/20"
                        >
                            {expanded ? '收起步骤' : '查看步骤'}
                        </button>
                        <button
                            onClick={() => {
                                setSessionExpired(false);
                                setIsAuthOpen(true);
                            }}
                            className="flex items-center gap-1.5 rounded-lg bg-warning px-3 py-1.5 text-[11px] font-bold text-primary-foreground transition-all hover:bg-warning/90"
                        >
                            <RefreshCw size={12} /> 打开登录窗口
                        </button>
                    </div>
                </div>
                <button
                    onClick={() => setDismissed(true)}
                    className="shrink-0 rounded-lg p-1 text-warning transition-colors hover:bg-warning/20 hover:text-warning"
                >
                    <X size={14} />
                </button>
            </div>
        </div>
    );
};
