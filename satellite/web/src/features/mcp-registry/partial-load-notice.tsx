import { AlertTriangleIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useI18n } from "@/lib/i18n-context"

/**
 * Slim inline notice for a partially loaded registry: some per-instance
 * listings failed (disconnected service, OAuth pending) while the rest of
 * the catalog is usable. Not a page error — content stays visible.
 */
export function PartialLoadNotice({ count, onRefresh }: { count: number; onRefresh: () => void }) {
  const { t } = useI18n()
  if (count <= 0) return null
  return (
    <div className="flex items-center gap-2 border-b border-amber-500/20 bg-amber-500/10 px-4 py-1.5 text-xs text-amber-700 dark:text-amber-400">
      <AlertTriangleIcon className="size-3.5 shrink-0" />
      <span className="min-w-0 flex-1 truncate">{t("partialInstancesFailed", { count })}</span>
      <Button size="sm" variant="ghost" className="h-6 px-2 text-xs" onClick={onRefresh}>
        {t("refresh")}
      </Button>
    </div>
  )
}
