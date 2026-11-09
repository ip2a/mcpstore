import { useCallback, useEffect, useState } from "react"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import { DownloadIcon, RotateCwIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { isDesktopShell, formatReleaseVersion } from "@/features/settings/updater"
import { useI18n } from "@/lib/i18n-context"

const FIRST_CHECK_MAX_DELAY_MS = 30 * 60 * 1000
const CHECK_INTERVAL_MS = 12 * 60 * 60 * 1000

type BannerPhase = "idle" | "downloading" | "ready"

// Auto-discovery half of the updater: silent checks + a dismissable banner
// with its own download flow. Independent of the manual button in settings
// (settings re-checks on click); no shared state machine on purpose.
export function UpdateBanner() {
  const { t } = useI18n()
  const [update, setUpdate] = useState<Update | null>(null)
  const [dismissed, setDismissed] = useState(false)
  const [phase, setPhase] = useState<BannerPhase>("idle")
  const [progress, setProgress] = useState(0)

  useEffect(() => {
    if (!isDesktopShell) return
    let stopped = false
    let timer: ReturnType<typeof setInterval> | undefined
    const checkSilently = async () => {
      try {
        const found = await check()
        if (found != null && !stopped) {
          setUpdate(found)
          if (timer) clearInterval(timer)
        }
      } catch {
        // Silent path: a failed probe must never disturb the user.
      }
    }
    // Stagger the first probe (0-30 min) so launches don't sync up, then poll.
    const first = setTimeout(() => {
      void checkSilently()
      timer = setInterval(() => void checkSilently(), CHECK_INTERVAL_MS)
    }, Math.random() * FIRST_CHECK_MAX_DELAY_MS)
    return () => {
      stopped = true
      clearTimeout(first)
      if (timer) clearInterval(timer)
    }
  }, [])

  const download = useCallback(async () => {
    if (!update) return
    setPhase("downloading")
    try {
      let total = 0
      let received = 0
      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength != null) total = event.data.contentLength
        if (event.event === "Progress" && total > 0) {
          received += event.data.chunkLength
          setProgress(Math.min(100, Math.round((received / total) * 100)))
        }
      })
      setPhase("ready")
    } catch (error) {
      setPhase("idle")
      toast.error(t("updateDownloadFailed", { error: error instanceof Error ? error.message : String(error) }))
    }
  }, [update, t])

  const restart = useCallback(() => void relaunch(), [])

  if (!isDesktopShell || !update || dismissed) return null

  return (
    <div className="fixed right-4 top-4 z-50 w-80 rounded-lg border bg-card p-4 shadow-md">
      <p className="text-sm font-medium">{t("updateAvailable", { version: formatReleaseVersion(update.version) })}</p>
      {phase === "downloading" ? (
        <p className="mt-2 flex items-center gap-2 text-sm text-muted-foreground">
          <RotateCwIcon className="size-4 animate-spin" />
          {t("downloadingUpdate", { progress: `${progress}%` })}
        </p>
      ) : phase === "ready" ? (
        <div className="mt-3 flex justify-end gap-2">
          <Button type="button" size="sm" onClick={restart}>
            <RotateCwIcon data-icon="inline-start" />
            {t("restartToUpdate")}
          </Button>
        </div>
      ) : (
        <div className="mt-3 flex justify-end gap-2">
          <Button type="button" size="sm" variant="ghost" onClick={() => setDismissed(true)}>
            {t("later")}
          </Button>
          <Button type="button" size="sm" onClick={() => void download()}>
            <DownloadIcon data-icon="inline-start" />
            {t("downloadUpdate")}
          </Button>
        </div>
      )}
    </div>
  )
}
