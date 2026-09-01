// @ts-check
import { defineConfig } from 'astro/config';
import tailwindcss from '@tailwindcss/vite';

/* No UI framework integration: the site ships plain HTML, a stylesheet, and
   about 8 kB of hand-written JavaScript. The React, GSAP, Lenis and
   framer-motion dependencies the previous version carried were doing work that
   CSS and one IntersectionObserver do without a bundle. */
export default defineConfig({
  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'fr'],
    routing: { prefixDefaultLocale: false },
  },

  vite: {
    plugins: [tailwindcss()],
  },
});
