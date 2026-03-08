import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte({
    compilerOptions: { hmr: !process.env.VITEST },
  })],
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.js'],
    alias: {
      // Ensure Svelte resolves to browser bundle in tests
      svelte: 'svelte',
    },
  },
  resolve: {
    conditions: process.env.VITEST ? ['browser'] : [],
  },
})
