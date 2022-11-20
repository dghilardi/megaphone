import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/room': {
        target: 'http://localhost:3040',
        changeOrigin: true,
        secure: false,
      },
      '/write': {
        target: 'http://localhost:3040',
        changeOrigin: true,
        secure: false,
      },
      '/read': {
        target: 'http://localhost:3000',
        changeOrigin: true,
        secure: false,
      }
    }
  }
})
