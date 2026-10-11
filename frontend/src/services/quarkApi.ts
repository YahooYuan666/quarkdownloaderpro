import { invoke } from '@tauri-apps/api/core';
import type {
    CapacityData,
    DownloadUrlData,
    QuarkResponse,
    RawFileItem,
    ShareFileNode,
    ShareTokenData,
    TaskResponse,
} from '../types/quark';

interface ShareDetailData {
    list: RawFileItem[];
}

interface SaveFilesData {
    task_id: string;
}

interface LogoutData {
    ok: boolean;
    logged_out: boolean;
    message: string;
    logout_request_error?: string;
    verify_error?: string;
    verify?: { endpoint?: string; status_code?: number; code?: string | number; message?: string; nickname?: string };
}

// 所有 API 改走 Tauri 命令（本地 HTTP 服务已删除）。
// 扫码登录已由「网页登录窗口」取代，不再需要前端侧的二维码流程。
// 返回体为夸克上游 JSON 的逐字透传，类型与原 HTTP 版一致。
export const quarkApi = {
    // 分享链接
    getShareToken: async (pwdId: string, passcode: string) => {
        return invoke<QuarkResponse<ShareTokenData>>('share_token', { pwdId, passcode });
    },

    getShareDetail: async (pid: string, st: string, fid: string = '0', page: number = 1) => {
        return invoke<QuarkResponse<ShareDetailData>>('share_detail', {
            pwdId: pid,
            stoken: st,
            pdirFid: fid,
            page,
        });
    },

    // 文件操作
    saveFiles: async (pid: string, st: string, fids: string[], tokens: string[]) => {
        return invoke<QuarkResponse<SaveFilesData>>('share_save', {
            pwdId: pid,
            stoken: st,
            fidList: fids,
            fidTokenList: tokens,
        });
    },

    queryTask: async (taskId: string, retryIndex: number) => {
        return invoke<QuarkResponse<TaskResponse>>('query_task', { taskId, retryIndex });
    },

    getDownloadUrl: async (fids: string[]) => {
        return invoke<QuarkResponse<DownloadUrlData[]>>('file_download_url', { fids });
    },

    deleteFiles: async (fids: string[]) => {
        return invoke<QuarkResponse>('file_delete', { fids });
    },

    getMemberInfo: async () => {
        return invoke<QuarkResponse<CapacityData>>('member_info');
    },

    logout: async () => {
        return invoke<LogoutData>('logout_cmd');
    },
};

// 辅助函数
export const collectFiles = (nodes: ShareFileNode[], allFiles: ShareFileNode[] = []) => {
    for (const node of nodes) {
        if (!node.isDir) {
            allFiles.push(node);
        }
        if (node.children.length > 0) {
            collectFiles(node.children, allFiles);
        }
    }
    return allFiles;
};

export const parseShareUrl = (url: string) => {
    const cleanUrl = url.replace(/\[.*?\]/g, '').trim();
    let pwdId = '';
    let passcode = '';
    let pdirFid = '0';

    const m1 = cleanUrl.match(/\/s\/([a-zA-Z0-9]+)/);
    if (m1) pwdId = m1[1];

    const m2 = cleanUrl.match(/[?&](pwd|passcode|password|pw)=([a-zA-Z0-9]{4})/i);
    if (m2) passcode = m2[2];

    const m3 = cleanUrl.match(/#\/list\/share\/([a-zA-Z0-9]+)/);
    if (m3) pdirFid = m3[1];

    return { pwdId, passcode, pdirFid };
};
