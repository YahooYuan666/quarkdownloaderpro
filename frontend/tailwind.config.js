/** @type {import('tailwindcss').Config} */
export default {
    darkMode: ["class"],
    content: [
        "./index.html",
        "./src/**/*.{js,ts,jsx,tsx}",
    ],
    theme: {
        extend: {
            borderRadius: {
                lg: 'var(--radius)',
                md: 'calc(var(--radius) - 2px)',
                sm: 'calc(var(--radius) - 4px)'
            },
            /*
             * 只暴露语义令牌。刻意不重新映射 slate/indigo/emerald 等原始色板——
             * 之前按主题反转原始色阶的做法会让「文字色」与「背景色」互相打架，
             * 且任何未映射的档位（如 pink-500）会静默失效。组件一律用语义名。
             */
            colors: {
                canvas: 'hsl(var(--canvas))',
                background: 'hsl(var(--canvas))',
                foreground: 'hsl(var(--foreground))',
                card: {
                    DEFAULT: 'hsl(var(--card))',
                    foreground: 'hsl(var(--foreground))',
                },
                raised: {
                    DEFAULT: 'hsl(var(--raised))',
                    foreground: 'hsl(var(--foreground))',
                },
                popover: {
                    DEFAULT: 'hsl(var(--overlay))',
                    foreground: 'hsl(var(--foreground))',
                },
                muted: {
                    DEFAULT: 'hsl(var(--muted))',
                    foreground: 'hsl(var(--muted-foreground))',
                },
                primary: {
                    DEFAULT: 'hsl(var(--primary))',
                    hover: 'hsl(var(--primary-hover))',
                    foreground: 'hsl(var(--primary-foreground))',
                },
                secondary: {
                    DEFAULT: 'hsl(var(--muted))',
                    foreground: 'hsl(var(--foreground))',
                },
                accent: {
                    DEFAULT: 'hsl(var(--raised))',
                    foreground: 'hsl(var(--foreground))',
                },
                destructive: {
                    DEFAULT: 'hsl(var(--danger))',
                    foreground: 'hsl(var(--primary-foreground))',
                },
                success: {
                    DEFAULT: 'hsl(var(--success))',
                    soft: 'hsl(var(--success-soft))',
                    'on-soft': 'hsl(var(--success-on-soft))',
                },
                warning: {
                    DEFAULT: 'hsl(var(--warning))',
                    soft: 'hsl(var(--warning-soft))',
                    'on-soft': 'hsl(var(--warning-on-soft))',
                },
                danger: {
                    DEFAULT: 'hsl(var(--danger))',
                    soft: 'hsl(var(--danger-soft))',
                    'on-soft': 'hsl(var(--danger-on-soft))',
                },
                info: {
                    DEFAULT: 'hsl(var(--info))',
                    soft: 'hsl(var(--info-soft))',
                    'on-soft': 'hsl(var(--info-on-soft))',
                },
                hud: {
                    DEFAULT: 'hsl(var(--hud))',
                    foreground: 'hsl(var(--hud-foreground))',
                },
                border: 'hsl(var(--border))',
                input: 'hsl(var(--input))',
                ring: 'hsl(var(--ring))',
            },
            fontFamily: {
                sans: ['var(--font-ui)'],
                display: ['var(--font-display)'],
            },
            boxShadow: {
                card: '0 1px 2px 0 rgb(0 0 0 / 0.04), 0 1px 3px 0 rgb(0 0 0 / 0.06)',
                pop: '0 10px 30px -12px rgb(0 0 0 / 0.25)',
            },
            keyframes: {
                "accordion-down": {
                    from: { height: "0" },
                    to: { height: "var(--radix-accordion-content-height)" },
                },
                "accordion-up": {
                    from: { height: "var(--radix-accordion-content-height)" },
                    to: { height: "0" },
                },
            },
            animation: {
                "accordion-down": "accordion-down 0.2s ease-out",
                "accordion-up": "accordion-up 0.2s ease-out",
            },
        },
    },
    plugins: [require("tailwindcss-animate")],
}
