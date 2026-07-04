/* global process */
import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'url'

export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), '')
    const frontendPort = Number(env.SERVERKIT_FRONTEND_PORT) || 41921
    const apiTarget = (env.VITE_API_URL || 'http://localhost:47927/api/v1').replace(/\/api\/v1\/?$/, '')

    const rawAllowedHosts = (env.VITE_ALLOWED_HOSTS || '').trim()
    let allowedHosts
    if (rawAllowedHosts === 'all') {
        allowedHosts = true
    } else if (rawAllowedHosts) {
        allowedHosts = rawAllowedHosts.split(',').map(s => s.trim()).filter(Boolean)
    }

    return {
        plugins: [react()],
        resolve: {
            alias: {
                '@': fileURLToPath(new URL('./src', import.meta.url)),
                // Stable import path for plugin code:
                //   import { api, useAuth } from 'serverkit-sdk';
                // Internal restructures of src/plugins/sdk are invisible
                // to plugins as long as the named exports stay stable.
                'serverkit-sdk': fileURLToPath(new URL('./src/plugins/sdk/index.js', import.meta.url)),
            },
            // Force every `react` / `react-dom` / `react-router-dom`
            // import (host code AND plugin code loaded via
            // import.meta.glob) to resolve to one copy. Without this
            // Vite can hand plugins a different React instance if dep
            // optimization runs mid-session, producing the
            // "Invalid hook call / ReactCurrentDispatcher is null"
            // crash inside the contributions hook.
            //
            // Do NOT add resolve.alias entries for these — aliasing to
            // the package directory bypasses Vite's optimizeDeps and
            // serves raw ESM, which (combined with pre-bundled
            // react-dom) recreates the same two-copies problem from the
            // other direction.
            dedupe: ['react', 'react-dom', 'react-router-dom'],
        },
        optimizeDeps: {
            // Pre-bundle the renderer at server start instead of
            // discovering it lazily. Lazy discovery is what triggers the
            // mid-session re-bundle that strands the open browser tab
            // on a stale React URL.
            include: ['react', 'react-dom', 'react-router-dom'],
        },
        server: {
            port: frontendPort,
            ...(allowedHosts !== undefined ? { allowedHosts } : {}),
            proxy: {
                '/api': {
                    target: apiTarget,
                    changeOrigin: true,
                },
                '/socket.io': {
                    target: apiTarget,
                    changeOrigin: true,
                    ws: true,
                },
            },
            // Enable polling for WSL (Windows filesystem doesn't support inotify)
            watch: {
                usePolling: true,
                interval: 1000,
            },
        },
        css: {
            preprocessorOptions: {
                scss: {
                    // Silence Dart Sass deprecation warnings for @import and slash-div
                    // These are expected during migration from LESS and will be addressed
                    // when moving to @use/@forward module system
                    silenceDeprecations: ['import', 'slash-div', 'legacy-js-api', 'global-builtin', 'color-functions', 'strict-unary'],
                },
            },
        },
        build: {
            sourcemap: false,
            rollupOptions: {
                output: {
                    manualChunks: {
                        'vendor-react': ['react', 'react-dom', 'react-router-dom'],
                        'vendor-charts': ['recharts'],
                        'vendor-flow': ['@xyflow/react'],
                        'vendor-xterm': ['@xterm/xterm', '@xterm/addon-fit', '@xterm/addon-web-links'],
                        'vendor-icons': ['lucide-react'],
                    },
                },
            },
        },
    }
})
