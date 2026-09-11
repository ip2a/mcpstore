import { SettingsIcon } from "lucide-react"

import { JsonBlock } from "@/components/shared/json-block"
import { PageSkeleton } from "@/components/shared/page-states"
import { SectionHeading } from "@/components/shared/section-heading"
import { useI18n } from "@/lib/i18n-context"

/** Config reset target: the store globally / a specific agent scope. */
export type ResetTarget = { scope: "store" } | { scope: "agent"; agentId: string }

/** Read-only display of a single scope's config tree. */
export function ConfigDetailPane({
  loading,
  value,
}: {
  loading: boolean
  value: unknown
}) {
  const { t } = useI18n()
  if (loading) return <PageSkeleton />

  return (
    <section className="pb-2">
      <SectionHeading
        title={t("configuration")}
        titleAs="h2"
        actions={<SettingsIcon className="size-4 text-muted-foreground" />}
        className="border-b-0 pb-3"
      />
      <JsonBlock value={value} />
    </section>
  )
}
