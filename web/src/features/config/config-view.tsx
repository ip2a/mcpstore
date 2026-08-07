import { SettingsIcon } from "lucide-react"

import { JsonBlock } from "@/components/shared/json-block"
import { PageSkeleton } from "@/components/shared/page-states"
import { SectionHeading } from "@/components/shared/section-heading"
import { useI18n } from "@/lib/i18n-context"

/** 配置重置目标：store 全局 / 某个 agent 作用域。 */
export type ResetTarget = { scope: "store" } | { scope: "agent"; agentId: string }

/** 单个作用域配置树的只读展示。 */
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
