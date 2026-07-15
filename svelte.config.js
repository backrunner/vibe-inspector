import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    adapter: adapter({ fallback: 'index.html' }),
    alias: {
      $components: './src/lib/components',
      $domain: './src/lib/domain',
      $stores: './src/lib/stores',
    },
  },
};

export default config;
