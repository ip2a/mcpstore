import type { UiLanguage } from "@/lib/api"
import {
  dictionaries,
  type I18nKey,
  type ResolvedLanguage,
} from "@/generated/i18n-dictionaries"

export type { I18nKey, ResolvedLanguage }
export { dictionaries }

export function browserLanguage(): ResolvedLanguage {
  if (typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")) return "zh"
  return "en"
}

export function resolveLanguage(language: UiLanguage | undefined): ResolvedLanguage {
  if (language === "zh" || language === "en") return language
  return browserLanguage()
}

export function translate(
  language: ResolvedLanguage,
  key: string,
  vars?: Record<string, string | number | null | undefined>,
) {
  let text: string = dictionaries[language][key as I18nKey] || dictionaries.en[key as I18nKey] || key
  if (!vars) return text
  for (const [name, value] of Object.entries(vars)) {
    text = text.replaceAll(`{${name}}`, String(value ?? ""))
  }
  return text
}
