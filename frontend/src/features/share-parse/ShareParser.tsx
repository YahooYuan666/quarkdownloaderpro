import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Loader2, Link2, Zap, X, Upload } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useQuarkStore } from '../../store/useQuarkStore';
import { quarkApi, parseShareUrl, collectFiles } from '../../services/quarkApi';
import type { RawFileItem, ShareFileNode } from '../../types/quark';
import { getErrorMessage, cn } from '../../utils';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { SearchContainer, GradientGlow } from '../../components/ui/card';
import { ActionBar } from '../download/ActionBar';
import { motion, AnimatePresence } from 'framer-motion';
import jsqr from 'jsqr';

type ParseProgressReporter = (list: RawFileItem[], depth: number, page: number) => void;

const isDirectory = (item: RawFileItem) => item.dir || item.file_type === 0 || item.obj_category === 'dir';

export const ShareParser: React.FC = () => {
    const [url, setUrl] = useState('');
    const [qrImagePreview, setQrImagePreview] = useState<string | null>(null);
    const [qrDecoding, setQrDecoding] = useState(false);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const qrDropZoneRef = useRef<HTMLDivElement>(null);
    const {
        isLoggedIn,
        setShareInfo,
        resetShareInfo,
        addLog,
        notify,
        shareInfo,
        setShareUrl
    } = useQuarkStore();
    const [loading, setLoading] = useState(false);

    const isParsed = shareInfo.allFiles.length > 0;

    // QR 码解码：图片 URL（blob: 或 data:） -> 字符串
    const decodeQrFromSrc = useCallback(async (src: string): Promise<string | null> => {
        return new Promise((resolve) => {
            const img = new window.Image();
            img.onload = () => {
                // 缩放到 max 1024px 保证性能
                const maxDim = 1024;
                let { width, height } = img;
                if (width > maxDim || height > maxDim) {
                    const scale = Math.min(maxDim / width, maxDim / height);
                    width = Math.round(width * scale);
                    height = Math.round(height * scale);
                }
                const canvas = document.createElement('canvas');
                canvas.width = width;
                canvas.height = height;
                const ctx = canvas.getContext('2d');
                if (!ctx) {
                    resolve(null);
                    return;
                }
                ctx.drawImage(img, 0, 0, width, height);
                const imageData = ctx.getImageData(0, 0, width, height);
                const result = jsqr(imageData.data, width, height);
                resolve(result?.data ?? null);
            };
            img.onerror = () => resolve(null);
            img.src = src;
        });
    }, []);

    // 从解码文本中提取夸克分享链接
    const extractQuarkShareUrl = (text: string): string | null => {
        // 匹配 pan.quark.cn/s/xxx 格式，可能带 #/list/share 或 ?pwd=xxx
        const match = text.match(/https?:\/\/pan\.quark\.cn\/s\/[A-Za-z0-9]+[^\s]*/);
        return match?.[0] ?? null;
    };

    // 核心处理函数：给入图片 src 进行解码与解析
    const processQrImageSrc = useCallback(async (src: string) => {
        setQrDecoding(true);
        setQrImagePreview(src);
        try {
            const decoded = await decodeQrFromSrc(src);
            if (decoded) {
                const shareUrl = extractQuarkShareUrl(decoded);
                if (shareUrl) {
                    setUrl(shareUrl);
                    setShareUrl(shareUrl);
                    notify('已从二维码识别链接', 'success');
                } else {
                    notify('二维码内容不是夸克分享链接', 'warn');
                }
            } else {
                notify('未能识别二维码，请尝试更清晰的图片', 'error');
            }
        } finally {
            setQrDecoding(false);
        }
    }, [decodeQrFromSrc, notify, setShareUrl]);

    const handleQrFile = async (file: File) => {
        if (!file.type.startsWith('image/')) {
            notify('请选择图片文件', 'warn');
            return;
        }
        const src = URL.createObjectURL(file);
        await processQrImageSrc(src);
    };

    // 监听 Tauri 原生拖放事件（从 Windows 桌面/资源管理器拖入文件）
    useEffect(() => {
        let unlisten: (() => void) | null = null;
        try {
            getCurrentWebview().onDragDropEvent(async (event) => {
                if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
                    const filePath = event.payload.paths[0];
                    const lower = filePath.toLowerCase();
                    if (/\.(png|jpe?g|webp|bmp|gif)$/.test(lower)) {
                        setQrDecoding(true);
                        try {
                            const dataUrl = await invoke<string>('read_image_data_url', { path: filePath });
                            await processQrImageSrc(dataUrl);
                        } catch (e) {
                            notify(`读取拖入图片失败: ${getErrorMessage(e)}`, 'error');
                        } finally {
                            setQrDecoding(false);
                        }
                    } else {
                        notify('拖入的文件不是常见图片格式', 'warn');
                    }
                }
            }).then((fn) => { unlisten = fn; }).catch(() => {});
        } catch {
            // 非 Tauri 环境（如纯浏览器）
        }

        return () => {
            if (unlisten) unlisten();
        };
    }, [notify, processQrImageSrc]);

    const handleDragOver = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
    };

    const handleDrop = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        const file = e.dataTransfer.files[0];
        if (file) handleQrFile(file);
    };

    const handlePaste = (e: React.ClipboardEvent) => {
        const items = e.clipboardData.items;
        for (let i = 0; i < items.length; i++) {
            if (items[i].type.startsWith('image/')) {
                const file = items[i].getAsFile();
                if (file) {
                    e.preventDefault();
                    handleQrFile(file);
                    break;
                }
            }
        }
        // 也处理 clipboardData.files（某些浏览器）
        const files = e.clipboardData.files;
        if (files.length > 0 && files[0].type.startsWith('image/')) {
            e.preventDefault();
            handleQrFile(files[0]);
        }
    };

    const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (file) handleQrFile(file);
        // 重置 input 值以便同一文件可再次选择
        if (e.target) e.target.value = '';
    };

    const clearQrPreview = () => {
        setQrImagePreview(null);
    };

    // 键盘支持：Delete 清除预览，聚焦时 Ctrl+V 粘贴
    useEffect(() => {
        const el = qrDropZoneRef.current;
        if (!el) return;
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Delete' || e.key === 'Backspace') {
                if (qrImagePreview) {
                    e.preventDefault();
                    clearQrPreview();
                }
            }
        };
        el.addEventListener('keydown', onKeyDown);
        return () => el.removeEventListener('keydown', onKeyDown);
    }, [qrImagePreview]);

    const getFileListPaged = async (
        pid: string,
        st: string,
        fid: string = '0',
        depth: number = 0,
        onPageLoaded?: ParseProgressReporter
    ) => {
        let all: RawFileItem[] = [];
        let page = 1;
        let total = Infinity;

        while (all.length < total) {
            const res = await quarkApi.getShareDetail(pid, st, fid, page);
            if (res.status !== 200 || !res.data) {
                throw new Error(res.message || '获取文件列表失败');
            }

            const list = (res.data.list || []) as RawFileItem[];
            if (res.metadata?._total !== undefined) {
                total = res.metadata._total;
            } else if (list.length === 0) {
                break;
            }

            all = [...all, ...list];
            onPageLoaded?.(list, depth, page);

            if (list.length < 50) break;
            page++;
        }
        return all;
    };

    const getFileTreeRecursive = async (
        pid: string,
        st: string,
        fid: string = '0',
        depth: number = 0,
        parentPath: string = '',
        onPageLoaded?: ParseProgressReporter
    ): Promise<ShareFileNode[]> => {
        const list = await getFileListPaged(pid, st, fid, depth, onPageLoaded);
        const nodes: ShareFileNode[] = [];

        for (const item of list) {
            const isDir = isDirectory(item);
            const path = parentPath ? `${parentPath}/${item.file_name}` : item.file_name;

            const node: ShareFileNode = {
                fid: item.fid,
                file_name: item.file_name,
                size: item.size || 0,
                format_type: item.format_type || '',
                updated_at: item.updated_at || item.l_updated_at || 0,
                share_fid_token: item.share_fid_token || '',
                isDir,
                depth,
                path,
                children: [],
                expanded: depth === 0,
            };

            if (isDir) {
                node.children = await getFileTreeRecursive(pid, st, item.fid, depth + 1, path, onPageLoaded);
            }
            nodes.push(node);
        }
        return nodes;
    };

    const handleParse = async () => {
        if (!url.trim()) return;
        if (!isLoggedIn) {
            const message = '请先登录账号';
            setShareInfo({ status: message, statusType: 'warn' });
            notify(message, 'warn');
            return;
        }
        setLoading(true);
        resetShareInfo();
        addLog(`开始解析: ${url.substring(0, 50)}...`);

        const parseStats = {
            scannedItems: 0,
            scannedFiles: 0,
            scannedDirs: 0,
            lastToastAt: 0,
        };

        const reportParseProgress: ParseProgressReporter = (list, depth, page) => {
            let fileCount = 0;
            let dirCount = 0;

            for (const item of list) {
                if (isDirectory(item)) dirCount += 1;
                else fileCount += 1;
            }

            parseStats.scannedItems += list.length;
            parseStats.scannedFiles += fileCount;
            parseStats.scannedDirs += dirCount;

            const message = `解析中：已扫描 ${parseStats.scannedItems} 项（文件 ${parseStats.scannedFiles}，目录 ${parseStats.scannedDirs}）`;
            setShareInfo({ status: message, statusType: 'info' });

            const now = Date.now();
            const shouldNotify = now - parseStats.lastToastAt > 1000 || depth === 0 || page === 1;
            if (shouldNotify) {
                notify(message, 'info');
                parseStats.lastToastAt = now;
            }
        };

        try {
            const { pwdId, passcode, pdirFid } = parseShareUrl(url);
            if (!pwdId) throw new Error('无法从链接中提取分享 ID');

            setShareInfo({ status: '正在获取访问令牌...', statusType: 'info' });
            notify('正在获取访问令牌...', 'info');
            const resToken = await quarkApi.getShareToken(pwdId, passcode);
            if (!resToken.data?.stoken) throw new Error(resToken.message || '获取访问令牌失败');

            const stoken = resToken.data.stoken;
            notify('正在解析目录结构...', 'info');
            const files = await getFileTreeRecursive(pwdId, stoken, pdirFid, 0, '', reportParseProgress);
            const allFiles = collectFiles(files);

            setShareInfo({
                pwdId,
                stoken,
                shareUrl: url,
                files,
                allFiles,
                status: `解析完成，共 ${allFiles.length} 个文件`,
                statusType: 'success'
            });
            notify(`解析成功，发现 ${allFiles.length} 个文件`, 'success');
            addLog(`解析成功，发现 ${allFiles.length} 个文件`, 'success');
        } catch (err: unknown) {
            const message = getErrorMessage(err);
            setShareInfo({ status: `解析失败: ${message}`, statusType: 'error' });
            addLog(`解析失败: ${message}`, 'error');
            notify(`解析失败: ${message}`, 'error');
        } finally {
            setLoading(false);
        }
    };

    // 初始化时从 store 恢复 URL
    useEffect(() => {
        if (shareInfo.shareUrl && !url) {
            setUrl(shareInfo.shareUrl);
        }
    }, [shareInfo.shareUrl, url]);

    // 输入时同步到 store
    const handleUrlChange = (newUrl: string) => {
        setUrl(newUrl);
        setShareUrl(newUrl);
    };

    return (
        <div className="relative space-y-3.5">
            <div className="relative group">
                <GradientGlow />
                <SearchContainer className="relative pr-2">
                    <div className="flex items-center pl-4 text-muted-foreground group-focus-within:text-primary transition-colors">
                        <Link2 size={20} />
                    </div>
                    <Input
                        value={url}
                        onChange={(e) => handleUrlChange(e.target.value)}
                        placeholder="粘贴夸克网盘分享链接..."
                        className="border-0 bg-transparent shadow-none focus-visible:ring-0 focus-visible:bg-transparent h-14 font-medium"
                        onKeyDown={(e) => e.key === 'Enter' && handleParse()}
                    />

                    <div className="flex items-center gap-2.5">
                        <Button
                            onClick={handleParse}
                            disabled={loading || !url.trim()}
                            variant="default"
                            size="xl"
                            className={cn(
                                "relative overflow-hidden rounded-xl px-10 font-bold flex items-center gap-2 transition-all duration-300",
                                isParsed
                                    ? "bg-primary text-primary-foreground shadow-sm"
                                    : "hover:scale-[1.02] active:scale-[0.98]"
                            )}
                        >
                            <AnimatePresence mode="wait">
                                {loading ? (
                                    <motion.div key="loading" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                                        <Loader2 size={18} className="animate-spin" />
                                    </motion.div>
                                ) : (
                                    <motion.div
                                        key={isParsed ? "success" : "idle"}
                                        initial={{ y: 5, opacity: 0 }}
                                        animate={{ y: 0, opacity: 1 }}
                                        className="flex items-center gap-2"
                                    >
                                        {isParsed ? <Zap size={18} className="fill-white animate-pulse" /> : <Zap size={18} />}
                                        <span>解析提取</span>
                                    </motion.div>
                                )}
                            </AnimatePresence>
                        </Button>
                    </div>
                </SearchContainer>
            </div>

            {/* QR 码图片输入区 */}
            <div
                ref={(el) => { qrDropZoneRef.current = el; }}
                className={cn(
                    "relative rounded-xl border p-3 transition-colors cursor-pointer",
                    qrImagePreview ? "border-primary/30 bg-primary/5" : "border-border/50 hover:border-primary/20"
                )}
                onDragOver={handleDragOver}
                onDrop={handleDrop}
                onPaste={handlePaste}
                onClick={() => fileInputRef.current?.click()}
                tabIndex={0}
                role="button"
                aria-label="点击或拖入二维码图片"
            >
                <input
                    ref={fileInputRef}
                    type="file"
                    accept="image/*"
                    capture="environment"
                    className="hidden"
                    onChange={handleFileSelect}
                    aria-hidden="true"
                />
                {qrImagePreview ? (
                    <div className="flex items-center gap-3">
                        <div className="relative w-16 h-16 flex-shrink-0 rounded-lg overflow-hidden border border-border">
                            <img src={qrImagePreview} alt="二维码预览" className="w-full h-full object-cover" />
                            {qrDecoding && (
                                <div className="absolute inset-0 flex items-center justify-center bg-black/50">
                                    <Loader2 size={20} className="animate-spin text-white" />
                                </div>
                            )}
                        </div>
                        <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium text-foreground truncate">已选择二维码图片</p>
                            <p className="text-xs text-muted-foreground truncate">
                                {qrDecoding ? '正在识别...' : '点击或拖入更换，按 Delete 删除'}
                            </p>
                        </div>
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            onClick={clearQrPreview}
                            aria-label="移除二维码图片"
                            className="text-muted-foreground hover:text-foreground"
                        >
                            <X size={18} />
                        </Button>
                    </div>
                ) : (
                    <div className="flex flex-col items-center justify-center py-4 text-center">
                        <div className="flex items-center gap-2 text-muted-foreground mb-2">
                            <Upload size={20} />
                            <span className="text-sm font-medium">或 拖入 / 粘贴 / 点击选择二维码图片</span>
                        </div>
                        <p className="text-xs text-muted-foreground/70">
                            支持截图、保存的分享码图片；识别后自动填入链接并解析
                        </p>
                        <kbd className="mt-2 px-2 py-1 text-xs bg-muted rounded border border-border font-mono">
                            Ctrl+V / Cmd+V 粘贴
                        </kbd>
                    </div>
                )}
            </div>

            <ActionBar />
        </div>
    );
};
