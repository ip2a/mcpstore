import { useCallback, useState } from "react"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import { DownloadIcon, RotateCwIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { useI18n } from "@/lib/i18n-context"

export const isDesktopShell = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window

type UpdaterPhase = "idle" | "checking" | "downloading" | "ready"

export function useAppUpdater() {
  const { t } = useI18n()
  const [phase, setPhase] = useState<UpdaterPhase>("idle")
  const [progress, setProgress] = useState(0)
  const [update, setUpdate] = useState<Update | null>(null)

  const checkForUpdate = useCallback(async () => {
    if (!isDesktopShell) return
    setPhase("checking")
    try {
      const result = await check()
      if (result == null) {
        setPhase("idle")
        toast.success(t("upToDate"))
        return
      }
      setUpdate(result)
      setPhase("downloading")
      let total = 0
      let received = 0
      await result.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength != null) total = event.data.contentLength
        if (event.event === "Progress" && total > 0) {
          received += event.data.chunkLength
          setProgress(Math.min(100, Math.round((received / total) * 100)))
        }
      })
      setPhase("ready")
      toast.success(t("updateDownloaded"))
    } catch (error) {
      setPhase("idle")
      toast.error(t("updateCheckFailed", { error: error instanceof Error ? error.message : String(error) }))
    }
  }, [t])

  const restartToUpdate = useCallback(() => {
    void relaunch()
  }, [])

  return { phase, progress, version: update?.version ?? null, checkForUpdate, restartToUpdate }
}

export function UpdateButton({ phase, progress, onCheck, onRestart }: {
  phase: UpdaterPhase
  progress: number
  onCheck: () => void
  onRestart: () => void
}) {
  const { t } = useI18n()
  if (phase === "ready") {
    return (
      <Button type="button" size="sm" onClick={onRestart}>
        <RotateCwIcon data-icon="inline-start" />
        {t("restartToUpdate")}
      </Button>
    )
  }
  return (
    <Button type="button" size="sm" variant="outline" disabled={phase !== "idle"} onClick={onCheck}>
      {phase !== "idle" ? (
        <RotateCwIcon data-icon="inline-start" className="animate-spin" />
      ) : (
        <DownloadIcon data-icon="inline-start" />
      )}
      {phase === "downloading" ? t("downloadingUpdate", { progress: `${progress}%` }) : t("checkUpdates")}
    </Button>
  )
}
