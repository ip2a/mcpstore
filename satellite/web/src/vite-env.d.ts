/// <reference types="vite/client" />

declare const __WEB_APP_VERSION__: string

declare module "@/vendor/ascii-ocean.js" {
  export function mountAsciiOcean(
    el: HTMLElement,
    opts?: Record<string, unknown>,
  ): { update: (opts: Record<string, unknown>) => void; destroy: () => void }
}

declare namespace React {
  namespace JSX {
    interface IntrinsicElements {
      "focus-switch": React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>
    }
  }
}
