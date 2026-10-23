import { SettingsIcon } from "lucide-react"

import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export function AppBottomBar({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { t } = useI18n()

  return (
    <footer className="flex h-8 shrink-0 items-center border-t bg-card/40">
      <button
        type="button"
        aria-label={t("settings")}
        className={cn(
          "flex w-48 shrink-0 items-center gap-2 px-4 text-sm text-muted-foreground transition-colors",
          "hover:bg-muted/60 hover:text-foreground",
        )}
        onClick={onOpenSettings}
      >
        <SettingsIcon aria-hidden="true" className="size-4 shrink-0" />
      </button>
    </footer>
  )
}
