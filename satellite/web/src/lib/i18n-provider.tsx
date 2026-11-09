import { useQuery } from "@tanstack/react-query"
import { useEffect, useMemo, useState, type ReactNode } from "react"

import { getMeta, type UiLanguage } from "@/lib/api"
import { parseHostsPayload } from "@/lib/api/hosts"
import { syncApiBaseFromHosts, tryInitFromConnectionHint } from "@/lib/api/backend"
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

  const [apiReady, setApiReady] = useState(false)

  useEffect(() => {
    // Try connection hint first (injected by daemon's web face)
    if (tryInitFromConnectionHint()) {
      setApiReady(true)
      return
    }

    // Fallback: use hosts config from meta
    const hosts = meta.data?.settings?.hosts
    if (!hosts) return
    syncApiBaseFromHosts(parseHostsPayload(hosts))
    setApiReady(true)
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

  // Config gate: don't mount children until the API base is initialized
  // (render-time readers like HomeHero would otherwise throw).
  if (!apiReady) {
    if (meta.isError) {
      return (
        <div className="grid h-dvh place-items-center text-sm text-destructive">
          failed to load meta: {String(meta.error)}
        </div>
      )
    }
    return null
  }

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
