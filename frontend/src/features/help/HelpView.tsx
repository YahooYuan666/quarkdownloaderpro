import React from 'react';
import {
    KeyRound, Link2, ListChecks, FolderOpen, AlertTriangle, Github,
    HelpCircle, MousePointerClick, ClipboardPaste, Globe, Wrench, ShieldAlert,
} from 'lucide-react';

/**
 * 使用说明：以「照着做就能用」为目标。
 * 重点写清楚获取 Cookie 的完整步骤（二维码登录不可用时的主要登录途径）。
 */

const Step: React.FC<{ n: number; title: string; children: React.ReactNode }> = ({ n, title, children }) => (
    <div className="flex gap-3">
        <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary text-xs font-black text-primary-foreground">
            {n}
        </span>
        <div className="min-w-0 flex-1">
            <p className="text-sm font-bold text-foreground">{title}</p>
            <div className="mt-1 space-y-1.5 text-sm leading-relaxed text-muted-foreground">{children}</div>
        </div>
    </div>
);

const Card: React.FC<{ icon: React.ElementType; title: string; children: React.ReactNode }> = ({ icon: Icon, title, children }) => (
    <section className="rounded-2xl border border-border bg-card p-6 shadow-card">
        <div className="mb-4 flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
                <Icon size={18} />
            </span>
            <h3 className="text-base font-bold text-foreground">{title}</h3>
        </div>
        {children}
    </section>
);

const Kbd: React.FC<{ children: React.ReactNode }> = ({ children }) => (
    <code className="rounded-md border border-border bg-muted px-1.5 py-0.5 text-xs font-semibold text-foreground">{children}</code>
);

export const HelpView: React.FC = () => {
    return (
        <div className="flex-1 space-y-5 overflow-y-auto pb-4">
            <header className="space-y-1">
                <h2 className="text-xl font-black text-foreground">使用说明</h2>
                <p className="text-sm text-muted-foreground">按下面的顺序做一遍即可完成一次完整下载。</p>
            </header>

            {/* 登录 —— 全文重点 */}
            <Card icon={KeyRound} title="第一步：登录账号">
                <p className="mb-4 text-sm leading-relaxed text-muted-foreground">
                    推荐用「网页登录」：软件会打开一个夸克官方网页，你在里面正常登录即可，<b className="text-foreground">不需要接触 Cookie</b>。
                </p>

                <div className="space-y-4">
                    <Step n={1} title="点左下角「立即登录」">
                        <p>在弹出的窗口里选「网页登录」，点「打开登录窗口」。</p>
                    </Step>
                    <Step n={2} title="在打开的窗口里正常登录">
                        <p>这个窗口就是夸克官网，可以用：扫码、短信验证码、账号密码——任意一种都行。</p>
                        <p className="text-xs">窗口会一直显示在最前面，不会被主界面挡住。</p>
                    </Step>
                    <Step n={3} title="登录成功后自动完成">
                        <p>软件会自己检测到登录成功，把凭据加密保存好，然后自动关闭登录窗口。你不需要做别的。</p>
                    </Step>
                </div>

                <details className="mt-5 rounded-xl border border-border bg-muted/50 p-4">
                    <summary className="cursor-pointer text-sm font-bold text-foreground">
                        备选：手动复制 Cookie（网页打不开时用）
                    </summary>
                    <div className="mt-3 space-y-3">
                        <p className="text-xs leading-relaxed text-muted-foreground">
                            如果登录窗口显示空白或无法加载，说明当前网络访问夸克官网受限，可以手动复制 Cookie：
                        </p>
                        <div className="space-y-3">
                            <Step n={1} title="在浏览器里登录夸克网盘">
                                <p>用浏览器打开 <a href="https://pan.quark.cn" target="_blank" rel="noreferrer" className="font-bold text-primary underline">pan.quark.cn</a> 并登录。</p>
                            </Step>
                            <Step n={2} title="打开开发者工具并刷新">
                                <p>按 <Kbd>F12</Kbd>，切到「<Kbd>网络</Kbd>」（Network）标签，按 <Kbd>F5</Kbd> 刷新。</p>
                            </Step>
                            <Step n={3} title="选中第一条请求">
                                <p>点列表里<b className="text-foreground">第一条</b>（类型 document、状态 200，名称通常叫 <Kbd>list</Kbd>）。</p>
                            </Step>
                            <Step n={4} title="复制 Cookie">
                                <p>右侧「<Kbd>标头</Kbd>」→「<Kbd>请求标头</Kbd>」→ 找到 <Kbd>Cookie:</Kbd> 那行。</p>
                                <p className="flex items-start gap-1.5 rounded-lg bg-warning-soft px-3 py-2 text-xs font-medium text-warning-on-soft">
                                    <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                    <span>在这一行上<b>点右键 → 选「复制值」</b>。不要手动拖选，容易漏字符。</span>
                                </p>
                            </Step>
                            <Step n={5} title="回到软件粘贴">
                                <p>「立即登录」→「手动输入 Cookie」→ 粘贴 →「保存并验证」。成功会显示你的账号昵称。</p>
                            </Step>
                        </div>
                    </div>
                </details>

                <p className="mt-4 flex items-start gap-2 rounded-xl bg-danger-soft px-4 py-3 text-xs font-medium leading-relaxed text-danger-on-soft">
                    <ShieldAlert size={15} className="mt-0.5 shrink-0" />
                    <span>无论用哪种方式，登录凭据都只保存在本机的系统加密凭据库里，不会写进普通文件、也不会上传。</span>
                </p>
            </Card>

            {/* 解析下载 */}
            <Card icon={Link2} title="第二步：解析分享链接并下载">
                <div className="space-y-4">
                    <Step n={1} title="粘贴分享链接">
                        <p>把 <Kbd>https://pan.quark.cn/s/...</Kbd> 形式的分享链接粘到输入框，点「解析提取」。带提取码的链接可直接整串粘贴，软件会自动识别。</p>
                    </Step>
                    <Step n={2} title="勾选要下载的文件">
                        <p>在文件列表里勾选目标文件；文件夹可以点开查看里面的内容，也可以勾选整个文件夹。</p>
                        <p>下方操作条会实时显示「已选 N 个、共多少容量」。</p>
                    </Step>
                    <Step n={3} title="点「开始任务」">
                        <p>任务会提交到后台执行，自动完成「转存到网盘 → 取直链 → 删除转存 → 下载到本地」的全过程。</p>
                        <p className="text-xs">转存是夸克获取下载直链的必要步骤；用完的临时文件会被自动删除，不会在你的网盘里留垃圾。</p>
                    </Step>
                    <Step n={4} title="在「下载任务」页看进度">
                        <p>点左侧「下载任务」查看每个文件的进度、速度、剩余时间。单个文件下载时会另弹一个小窗，关掉小窗不会中断下载。</p>
                    </Step>
                    <Step n={5} title="完成后打开文件">
                        <p>任务完成后，在该任务上点「<FolderOpen size={13} className="inline" /> 打开所在文件夹」可直接在系统文件管理器里定位文件。</p>
                    </Step>
                </div>
            </Card>

            {/* 任务与设置 */}
            <div className="grid gap-5 lg:grid-cols-2">
                <Card icon={ListChecks} title="任务管理">
                    <ul className="space-y-2.5 text-sm text-muted-foreground">
                        <li className="flex gap-2"><MousePointerClick size={15} className="mt-0.5 shrink-0 text-primary" /><span><b className="text-foreground">取消</b>：任务行上的「取消」停单个，「全部取消」停全部。</span></li>
                        <li className="flex gap-2"><MousePointerClick size={15} className="mt-0.5 shrink-0 text-primary" /><span><b className="text-foreground">重试</b>：失败的任务会标红并显示原因，点「重试」重下，不必重新解析。</span></li>
                        <li className="flex gap-2"><ClipboardPaste size={15} className="mt-0.5 shrink-0 text-primary" /><span><b className="text-foreground">中断保护</b>：网络抖动会自动重试并续传；每个分片都会校验字节数，不会出现「看着下完了其实打不开」的文件。</span></li>
                    </ul>
                </Card>

                <Card icon={Wrench} title="软件设置">
                    <ul className="space-y-2.5 text-sm text-muted-foreground">
                        <li><b className="text-foreground">下载目录</b>：自定义保存位置，「记住上次目录」可让每次选择自动沿用。</li>
                        <li><b className="text-foreground">同名文件</b>：可选自动改名、跳过或每次询问。</li>
                        <li><b className="text-foreground">外观</b>：跟随系统 / 浅色 / 深色三选。</li>
                        <li><b className="text-foreground">分片数与并行数</b>：分片越多单文件越快，但过高可能被网盘限速，默认值已适配大多数情况。</li>
                        <li><b className="text-foreground">下载完成时显示完成窗</b>：不想要完成提示可关掉。</li>
                    </ul>
                </Card>
            </div>

            {/* 常见问题 */}
            <Card icon={HelpCircle} title="常见问题">
                <dl className="space-y-3.5 text-sm">
                    <div>
                        <dt className="font-bold text-foreground">提示「登录状态已失效」怎么办？</dt>
                        <dd className="mt-0.5 text-muted-foreground">说明 Cookie 过期了。按第一步重新复制一次即可，软件会在失效时弹出带步骤的提示条。</dd>
                    </div>
                    <div>
                        <dt className="font-bold text-foreground">解析失败或提示链接无效？</dt>
                        <dd className="mt-0.5 text-muted-foreground">先确认链接能在浏览器里正常打开；若浏览器可打开而软件不行，多半是登录失效。</dd>
                    </div>
                    <div>
                        <dt className="font-bold text-foreground">下载到一半失败了？</dt>
                        <dd className="mt-0.5 text-muted-foreground">在任务行点「重试」。若反复失败，可在设置里把「单文件分片数」调小（如 8）再试。</dd>
                    </div>
                    <div>
                        <dt className="font-bold text-foreground">找不到下载的文件？</dt>
                        <dd className="mt-0.5 text-muted-foreground">到「下载任务」页点「打开所在文件夹」；也可在设置里查看当前下载目录。</dd>
                    </div>
                    <div>
                        <dt className="font-bold text-foreground">二维码登录总是取不到？</dt>
                        <dd className="mt-0.5 text-muted-foreground">先用 Cookie 登录把软件跑起来。二维码依赖夸克官方接口，网络环境波动时会失败。</dd>
                    </div>
                </dl>
            </Card>

            {/* 致谢（简要） */}
            <footer className="rounded-2xl border border-border bg-card px-6 py-5 shadow-card">
                <div className="flex items-start gap-3">
                    <Github size={18} className="mt-0.5 shrink-0 text-muted-foreground" />
                    <div className="text-sm leading-relaxed text-muted-foreground">
                        <p>
                            本软件的夸克解析思路来自开源项目
                            <a href="https://github.com/muyan556/gopeed-extension-quark" target="_blank" rel="noreferrer" className="mx-1 font-bold text-primary underline">gopeed-extension-quark</a>
                            与
                            <a href="https://github.com/OP404OP/quarkdownloaderpro" target="_blank" rel="noreferrer" className="mx-1 font-bold text-primary underline">QuarkDownloaderPro</a>
                            ，在此感谢原作者的无私分享。
                        </p>
                        <p className="mt-1 text-xs">
                            本软件仅供技术学习与交流使用。请勿用于任何非法用途，使用过程中的一切行为需自行承担法律责任。
                        </p>
                    </div>
                </div>
            </footer>

            <p className="flex items-center justify-center gap-2 pb-2 text-xs text-muted-foreground">
                <Globe size={13} /> 所有数据仅保存在本机，不上传任何服务器
            </p>
        </div>
    );
};
