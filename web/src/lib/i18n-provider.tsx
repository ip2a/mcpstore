import { useQuery } from "@tanstack/react-query"
import { useEffect, useMemo, useState, type ReactNode } from "react"

import { getMeta, type UiLanguage } from "@/lib/api"
import { parseHostsPayload } from "@/lib/api/hosts"
import { syncApiBaseFromHosts } from "@/lib/api/backend"
import { I18nContext, type I18nContextValue } from "@/lib/i18n-context"
import { resolveLanguage, translate } from "@/lib/i18n-core"
import { queryKeys } from "@/lib/query-keys"

export function I18nProvider({ children }: { children: ReactNode }) {
  const meta = useQuery({ queryKey: queryKeys.meta, queryFn: getMeta })
  const [languageOverride, setLanguageOverride] = useState<UiLanguage | null>(null)
  const languageSetting = languageOverride ?? meta.data?.settings?.language ?? "auto"
  const language = resolveLanguage(languageSetting)

  useEffect(() => {
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en"
  }, [language])

  useEffect(() => {
    if (!meta.data?.settings?.hosts) return
    syncApiBaseFromHosts(parseHostsPayload(meta.data.settings.hosts))
  }, [meta.data?.settings?.hosts])

  const value = useMemo<I18nContextValue>(
    () => ({
      language,
      languageSetting,
      setLanguageOverride,
      t: (key, vars) => translate(language, key, vars),
    }),
    [language, languageSetting],
  )

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
