import { defineConfig } from 'vite';

export default defineConfig({
  base: '/',
  build: {
    target: 'esnext',
    minify: true,
    sourcemap: false,
    rollupOptions: {
      onwarn(warning, warn) {
        // wasm-bindgen emits this binding for js_sys::eval; generated code.
        if (warning.code === 'EVAL' && warning.id?.endsWith('/src/wasm/mcpstore_desktop.js')) {
          return;
        }
        warn(warning);
      },
    },
  },
  server: { port: 1831, strictPort: true },
});
