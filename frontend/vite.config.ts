import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

const backendPort = process.env.GH_INBOX_PORT ?? "3000";

export default defineConfig({
	plugins: [
		svelte({
			compilerOptions: { hmr: !process.env.VITEST },
		}),
	],
	server: {
		proxy: {
			"/api": {
				target: `http://127.0.0.1:${backendPort}`,
				changeOrigin: true,
			},
		},
	},
	test: {
		environment: "jsdom",
		setupFiles: ["./src/test-setup.ts"],
		alias: {
			svelte: "svelte",
		},
	},
	resolve: {
		...(process.env.VITEST && { conditions: ["browser"] }),
	},
});
