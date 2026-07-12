import { createContext, useContext } from "react"

import type { ResolvedLanguage } from "@/lib/i18n-core"
import type { UiLanguage } from "@/lib/api"

export type I18nContextValue = {
  language: ResolvedLanguage
  languageSetting: UiLanguage
  setLanguageOverride: (language: UiLanguage | null) => void
  t: (key: string, vars?: Record<string, string | number | null | undefined>) => string
}

export const I18nContext = createContext<I18nContextValue | null>(null)

export function useI18n() {
  const value = useContext(I18nContext)
  if (!value) throw new Error("useI18n must be used within I18nProvider")
  return value
}
