/**
 * 开发预览用的 Tauri IPC 模拟层（仅 `import.meta.env.DEV` 且无真实 Tauri 时加载）。
 *
 * 目的：在普通浏览器里打开 Vite 开发服务器即可预览/校对全部界面，
 * 不必每次构建安装包。产线构建里这段是死代码，会被 tree-shake 掉。
 *
 * 注意：这里返回的是**假数据**，只用于看界面，不代表真实后端行为。
 */

type MockArgs = Record<string, unknown> | undefined;
type Invoke = (args?: MockArgs) => Promise<unknown>;

const now = Date.now();

const fakeFiles = [
  {
    fid: 'dir-1', share_fid_token: 't-dir-1', file_name: '【软件】示例安装包 合集', dir: true, file_type: 0,
    size: 0, updated_at: now - 86400000 * 3, isDir: true,
    children: [
      {
        fid: 'file-4', share_fid_token: 't-f4', file_name: '（安卓）示例版本-v2.1.0.apk', dir: false,
        file_type: 1, size: 42_000_000, updated_at: now - 86400000 * 3, isDir: false, children: [],
      },
    ],
  },
  {
    fid: 'file-1', share_fid_token: 't-f1', file_name: '【mac】示例软件-0.2.3-macos-arm64.dmg', dir: false,
    file_type: 1, size: 58_900_000, updated_at: now - 86400000 * 3, isDir: false, children: [],
  },
  {
    fid: 'file-2', share_fid_token: 't-f2', file_name: '【win】示例软件-0.2.3-windows-x64-portable.zip', dir: false,
    file_type: 1, size: 28_400_000, updated_at: now - 86400000 * 3, isDir: false, children: [],
  },
  {
    fid: 'file-3', share_fid_token: 't-f3', file_name: '使用说明与更新日志（超长文件名测试用，用来检查会不会把布局撑破）.txt', dir: false,
    file_type: 1, size: 2_400, updated_at: now - 86400000 * 3, isDir: false, children: [],
  },
];

const fakeTasks = [
  {
    id: 'task-1', filename: '【win】示例软件-0.2.3-windows-x64-portable.zip', size: 28_400_000,
    status: 'downloading', downloaded: 17_300_000, speed: 4_100_000,
  },
  {
    id: 'task-2', filename: '【mac】示例软件-0.2.3-macos-arm64.dmg', size: 58_900_000,
    status: 'queued', downloaded: 0, speed: 0,
  },
  {
    id: 'task-3', filename: '示例素材包.zip', size: 12_000_000,
    status: 'done', downloaded: 12_000_000, speed: 0, savedPath: 'F:\\Downloads\\示例素材包.zip',
  },
  {
    id: 'task-4', filename: '已损坏的下载示例.7z', size: 8_000_000,
    status: 'failed', downloaded: 3_100_000, speed: 0,
    error: '分片 2 在 3 次重试后仍不完整: 已收 16384/3145728 字节',
  },
  {
    id: 'task-5', filename: '同名文件示例.zip', size: 1_000_000,
    status: 'skipped', downloaded: 0, speed: 0, error: '同名文件已存在，已跳过',
  },
];

const fakeSettings = {
  downloadDir: 'F:\\Downloads',
  lastDir: 'F:\\Downloads',
  rememberLastDir: true,
  duplicateStrategy: 'rename',
  appearance: 'system',
  progressWindowOnCompletion: true,
};

const handlers: Record<string, Invoke> = {
  cookie_state: async () => ({ has: true }),
  cookie_whoami: async () => ({ logged_in: true, nickname: '示例账号', message: '' }),
  cookie_set: async () => ({ ok: true, nickname: '示例账号', message: '登录成功' }),
  cookie_clear: async () => ({ ok: true }),
  settings_get: async () => fakeSettings,
  settings_set: async (args?: MockArgs) => {
    const patch = (args?.patch ?? {}) as Record<string, unknown>;
    Object.assign(fakeSettings, patch);
    return fakeSettings;
  },
  check_duplicates: async () => [],
  pick_folder: async () => 'F:\\Downloads',
  open_folder: async () => ({ ok: true }),
  open_file: async () => ({ ok: true }),
  member_info: async () => ({
    status: 200, code: 0, data: { use_capacity: 1_400_000_000, total_capacity: 10_737_418_240 },
  }),
  tasks_snapshot: async () => fakeTasks,
  share_token: async () => ({ status: 200, code: 0, data: { stoken: 'mock-stoken' }, message: 'ok' }),
  share_detail: async () => ({ status: 200, code: 0, data: { list: fakeFiles }, message: 'ok' }),
};

export function installTauriMock(): void {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (w.__TAURI_INTERNALS__) return;

  // OS 插件在模块求值期同步读取这份注入数据（缺失会让 platform() 抛错）
  (window as unknown as Record<string, unknown>).__TAURI_OS_PLUGIN_INTERNALS__ = {
    platform: 'windows',
    version: '10.0.26200',
    family: 'windows',
    type: 'windows',
    arch: 'x86_64',
    locale: 'zh-CN',
  };

  let callbackId = 0;
  const callbacks = new Map<number, unknown>();

  w.__TAURI_INTERNALS__ = {
    // @tauri-apps/api/window 在模块顶层读取 metadata；缺失会在导入期抛错
    metadata: {
      currentWindow: { label: 'main' },
      currentWebview: { label: 'main', windowLabel: 'main' },
    },
    transformCallback: (cb: unknown) => {
      const id = ++callbackId;
      callbacks.set(id, cb);
      return id;
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
    // 事件 API 的 unlisten 路径会读它
    unregisterListener: () => undefined,
    convertFileSrc: (p: string) => p,
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      // 事件插件：注册后不回推，避免 mock 触发假事件
      if (cmd.startsWith('plugin:event|')) {
        if (cmd === 'plugin:event|listen') return callbacks.size + 1;
        return null;
      }
      const handler = handlers[cmd];
      if (handler) return handler(args);
      // 窗口控制类命令：浏览器里无对应行为，静默返回成功
      if (cmd.startsWith('plugin:window|')) return null;
      console.warn('[dev-mock] 未模拟的命令:', cmd);
      return null;
    },
  };

  console.info('[dev-mock] 已启用 Tauri IPC 模拟：界面可直接在浏览器预览');
}
