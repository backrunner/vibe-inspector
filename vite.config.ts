import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  clearScreen: false,
  resolve: { dedupe: ['svelte'] },
  server: {
    port: 4173,
    strictPort: true,
  },
});
