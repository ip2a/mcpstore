import { useEffect, useRef } from "react"

import "@/components/shared/focus-switch-element.js"
import "@/components/shared/focus-switch.css"

type FocusSwitchElement = HTMLElement & {
  checked: boolean
}

type FocusHooks = {
  setX: (x: number) => void
}

declare global {
  interface Window {
    __focus?: FocusHooks
  }
}

/** Theme toggle built from local/参考项目/focus_switch.html (night = dark). */
export function FocusThemeSwitch({
  checked,
  onCheckedChange,
  label,
}: {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  label: string
}) {
  const ref = useRef<FocusSwitchElement | null>(null)

  useEffect(() => {
    const el = ref.current
    if (!el || el.checked === checked) return
    if (window.__focus?.setX) {
      window.__focus.setX(checked ? 1 : 0)
    } else {
      el.checked = checked
    }
  }, [checked])

  useEffect(() => {
    const el = ref.current
    if (!el) return

    const onChange = () => onCheckedChange(el.checked)
    el.addEventListener("change", onChange)

    const btn = el.querySelector("button")
    if (btn) btn.setAttribute("aria-label", label)

    return () => el.removeEventListener("change", onChange)
  }, [label, onCheckedChange])

  return (
    <div className="focus-theme-switch">
      <focus-switch ref={ref} />
    </div>
  )
}
