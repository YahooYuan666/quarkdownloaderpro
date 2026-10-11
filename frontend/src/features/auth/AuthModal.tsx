import React, { useState, useEffect, useCallback } from 'react';
import { Globe, Key, ShieldCheck, Loader2, CheckCircle2, XCircle, ExternalLink, AlertTriangle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useQuarkStore } from '../../store/useQuarkStore';
import { getErrorMessage } from '../../utils';
import { Button } from '../../components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '../../components/ui/dialog';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '../../components/ui/tabs';

interface AuthModalProps {
    isOpen: boolean;
    onClose: () => void;
    onLogout: () => void;
}

type WebLoginState = 'idle' | 'waiting' | 'done' | 'timeout' | 'error';

/**
 * 登录弹窗。
 *
 * 首选「网页登录」：打开一个真实的夸克网页窗口，用户用扫码 / 短信 / 密码
 * 任意方式登录即可，应用自动读取登录凭据——不需要理解或复制 Cookie。
 * 备选手动粘贴 Cookie（用于网页登录不方便的场景）。
 */
export const AuthModal: React.FC<AuthModalProps> = ({ isOpen, onClose, onLogout }) => {
    const [activeTab, setActiveTab] = useState<'web' | 'cookie'>('web');
    const { isLoggedIn, setLoginState, addLog, notify } = useQuarkStore();

    const [webState, setWebState] = useState<WebLoginState>('idle');
    const [webMessage, setWebMessage] = useState('');
    const [cookieInput, setCookieInput] = useState('');
    const [isSaving, setIsSaving] = useState(false);

    const handleClose = useCallback(() => {
        // 关掉弹窗时一并关掉可能开着的登录窗口，避免留下孤儿窗口
        invoke('cancel_web_login').catch(() => {});
        setWebState('idle');
        onClose();
    }, [onClose]);

    // 监听登录窗口的回调事件
    useEffect(() => {
        const unDone = listen<{ nickname?: string }>('login://done', (event) => {
            const nickname = event.payload?.nickname ?? '';
            setWebState('done');
            setWebMessage(nickname ? `登录成功，欢迎回来，${nickname}` : '登录成功');
            setLoginState(true, nickname);
            addLog('网页登录成功', 'success');
            notify(nickname ? `登录成功，欢迎回来，${nickname}` : '登录成功', 'success');
            setTimeout(() => onClose(), 1200);
        });
        const unTimeout = listen('login://timeout', () => {
            setWebState('timeout');
            setWebMessage('登录窗口已超时关闭，可以重新打开再试一次');
        });
        return () => {
            unDone.then((fn) => fn());
            unTimeout.then((fn) => fn());
        };
    }, [setLoginState, addLog, notify, onClose]);

    const openWebLogin = async () => {
        setWebState('waiting');
        setWebMessage('已打开登录窗口，请在其中完成登录');
        try {
            await invoke('open_web_login');
        } catch (error: unknown) {
            setWebState('error');
            setWebMessage(getErrorMessage(error));
        }
    };

    const saveCookie = async () => {
        const trimmed = cookieInput.trim();
        if (!trimmed) return;
        setIsSaving(true);
        try {
            const saved = await invoke<{ ok: boolean; nickname: string; message: string }>('cookie_set', { cookie: trimmed });
            if (saved.ok) {
                setLoginState(true, saved.nickname);
                addLog('Cookie 已保存并验证通过', 'success');
                notify(`登录成功，欢迎回来${saved.nickname ? `，${saved.nickname}` : ''}`, 'success');
                handleClose();
            } else {
                setLoginState(false, '');
                notify(`Cookie 无效：${saved.message}`, 'error');
            }
        } catch (error: unknown) {
            setLoginState(false, '');
            notify(`Cookie 验证失败：${getErrorMessage(error)}`, 'error');
        } finally {
            setIsSaving(false);
        }
    };

    const resetCookie = () => {
        setCookieInput('');
        if (isLoggedIn) {
            onLogout();
            handleClose();
        } else {
            notify('请先填写 Cookie', 'info');
        }
    };

    return (
        <Dialog open={isOpen} onOpenChange={(open) => !open && handleClose()}>
            <DialogContent className="max-w-[460px] p-8">
                <DialogHeader className="mb-6">
                    <div className="flex items-center gap-3">
                        <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-primary/10 text-primary">
                            <ShieldCheck size={22} />
                        </div>
                        <div>
                            <DialogTitle>登录夸克账号</DialogTitle>
                            <DialogDescription>推荐用网页登录，不需要手动处理 Cookie</DialogDescription>
                        </div>
                    </div>
                </DialogHeader>

                <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as 'web' | 'cookie')}>
                    <TabsList className="w-full">
                        <TabsTrigger value="web" className="flex-1 gap-2">
                            <Globe size={15} /> 网页登录
                        </TabsTrigger>
                        <TabsTrigger value="cookie" className="flex-1 gap-2">
                            <Key size={15} /> 手动输入 Cookie
                        </TabsTrigger>
                    </TabsList>

                    <div className="mt-6 min-h-[240px]">
                        {/* ── 网页登录 ── */}
                        <TabsContent value="web" className="space-y-5 outline-none">
                            <div className="space-y-2.5 text-sm leading-relaxed text-muted-foreground">
                                <p>点下面的按钮会打开一个夸克官方网页窗口，你在里面正常登录即可：</p>
                                <ul className="ml-1 space-y-1.5">
                                    <li>· 可以用<b className="text-foreground">扫码</b>（窗口里就是官网的二维码，和浏览器里一样）</li>
                                    <li>· 也可以用<b className="text-foreground">短信验证码</b>或<b className="text-foreground">账号密码</b>登录</li>
                                    <li>· 登录成功后窗口会<b className="text-foreground">自动关闭</b>，凭据自动保存</li>
                                </ul>
                            </div>

                            <Button
                                onClick={openWebLogin}
                                disabled={webState === 'waiting' || webState === 'done'}
                                className="w-full h-12 rounded-xl font-bold"
                            >
                                {webState === 'waiting' ? <Loader2 size={16} className="animate-spin" /> : <Globe size={16} />}
                                {webState === 'waiting' ? '等待登录中…' : '打开登录窗口'}
                            </Button>

                            {webMessage && (
                                <div className={`flex items-start gap-2 rounded-xl px-4 py-3 text-xs font-medium leading-relaxed ${
                                    webState === 'done' ? 'bg-success-soft text-success-on-soft'
                                    : webState === 'error' || webState === 'timeout' ? 'bg-warning-soft text-warning-on-soft'
                                    : 'bg-primary/10 text-primary'
                                }`}>
                                    {webState === 'done' ? <CheckCircle2 size={14} className="mt-0.5 shrink-0" />
                                     : webState === 'error' ? <XCircle size={14} className="mt-0.5 shrink-0" />
                                     : <Loader2 size={14} className={`mt-0.5 shrink-0 ${webState === 'waiting' ? 'animate-spin' : ''}`} />}
                                    <span>{webMessage}</span>
                                </div>
                            )}

                            <p className="text-xs leading-relaxed text-muted-foreground">
                                如果窗口打开后是空白或打不开，说明当前网络访问夸克官网受限，请改用旁边的「手动输入 Cookie」。
                            </p>
                        </TabsContent>

                        {/* ── 手动输入 Cookie ── */}
                        <TabsContent value="cookie" className="space-y-5 outline-none">
                            <div className="space-y-2">
                                <label className="pl-1 text-xs font-bold uppercase tracking-widest text-muted-foreground">
                                    夸克网盘 Cookie
                                </label>
                                <textarea
                                    value={cookieInput}
                                    onChange={(e) => setCookieInput(e.target.value)}
                                    placeholder="粘贴完整的 Cookie 字符串…"
                                    className="min-h-[120px] w-full resize-none rounded-2xl border border-input bg-muted p-4 font-mono text-xs outline-none transition-all focus:border-primary focus:bg-card"
                                />
                            </div>
                            <div className="flex gap-3">
                                <Button
                                    onClick={saveCookie}
                                    disabled={isSaving || !cookieInput.trim()}
                                    className="h-11 flex-1 rounded-xl font-bold"
                                >
                                    {isSaving ? '验证中…' : '保存并验证'}
                                </Button>
                                <Button variant="outline" onClick={resetCookie} className="h-11 rounded-xl px-6 font-bold">
                                    重置
                                </Button>
                            </div>
                            <div className="flex items-start gap-2.5 rounded-2xl border border-primary/20 bg-primary/10 p-4">
                                <ExternalLink size={14} className="mt-0.5 shrink-0 text-primary" />
                                <p className="text-xs font-medium leading-relaxed text-primary">
                                    获取方法见「使用说明 → 第一步」，里面有逐步截图级的操作描述。
                                </p>
                            </div>
                            <p className="flex items-start gap-2 rounded-xl bg-warning-soft px-4 py-3 text-xs font-medium leading-relaxed text-warning-on-soft">
                                <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                <span>Cookie 等同于账号密码，请勿截图或发送给他人。保存时会立即校验，成功会显示账号昵称。</span>
                            </p>
                        </TabsContent>
                    </div>
                </Tabs>
            </DialogContent>
        </Dialog>
    );
};
