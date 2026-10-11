import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export type Appearance = 'system' | 'light' | 'dark';

// 票 09：三态外观应用（system/light/dark），跟随系统实时切换
export function applyAppearance(pref: Appearance): void {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
        const dark = pref === 'dark' || (pref === 'system' && mq.matches);
        document.documentElement.classList.toggle('dark', dark);
    };
    apply();
    // 先清后挂，保证重复调用不叠加监听
    mq.removeEventListener('change', apply);
    mq.addEventListener('change', apply);
}

export function useAppearanceBootstrap(): void {
    useEffect(() => {
        invoke<{ appearance?: Appearance }>('settings_get')
            .then((s) => applyAppearance(s.appearance ?? 'system'))
            .catch(() => applyAppearance('system'));
    }, []);
}
