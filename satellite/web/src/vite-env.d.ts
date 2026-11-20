/// <reference types="vite/client" />

declare const __WEB_APP_VERSION__: string

declare namespace React {
  namespace JSX {
    interface IntrinsicElements {
      "focus-switch": React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>
    }
  }
}
