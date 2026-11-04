import { PageEmpty } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { useI18n } from "@/lib/i18n-context"

/** Placeholder Scope workspace — content TBD. */
export function ScopeView() {
  const { t } = useI18n()

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("navAgents")}</h1>}
      />
      <PanelCard className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <PageEmpty
          className="flex-1"
          title={t("navAgents")}
          description={t("scopePlaceholderDescription")}
        />
      </PanelCard>
    </>
  )
}
