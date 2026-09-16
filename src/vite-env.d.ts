/// <reference types="vite/client" />

// `?inline` gives a data: URL, which is what the drag plugin wants for the
// drag image and what keeps the icon out of a second HTTP request.
declare module '*.png?inline' {
  const src: string;
  export default src;
}

interface ImportMetaEnv {
  readonly VITE_APP_VERSION: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
