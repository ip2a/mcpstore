import { PageEmpty } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { useI18n } from "@/lib/i18n-context"

export function PromptsView() {
  const { t } = useI18n()

  return (
    <>
      <div className="min-w-0 shrink-0 pl-4">
        <h1 className="truncate text-lg font-semibold">{t("promptRegistryTitle")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t("promptRegistryPlaceholder")}</p>
      </div>

      <PanelCard className="flex min-h-0 flex-1 flex-col">
        <PageEmpty title={t("prompts")} description={t("promptRegistryPlaceholder")} />
      </PanelCard>
    </>
  )
}
