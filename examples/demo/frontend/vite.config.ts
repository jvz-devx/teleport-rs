import { sveltekit } from '@sveltejs/kit/vite';
import { teleportVite } from '@teleport-rs/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		teleportVite({
			bindingsPath: 'src/lib/api/generated',
			generateOnStart: true,
		}),
		sveltekit(),
	],
});
