import { sveltekit } from '@sveltejs/kit/vite';
import { teleportVite } from '@teleport-rs/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		teleportVite({
			bindingsPath: 'src/lib/api/generated',
			generateOnStart: {
				command: ['cargo', 'run', '-p', 'teleport-demo', '--bin', 'server', '--', '--export-only'],
				cwd: '..',
			},
		}),
		sveltekit(),
	],
});
