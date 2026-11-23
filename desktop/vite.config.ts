import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    watch: { ignored: ['**/src-tauri/target/**'] },
    host: '127.0.0.1',
    strictPort: true,
    port: 1420,
  },
})
