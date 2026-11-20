import { BadgeCheckIcon, ScrollTextIcon } from "lucide-react"

import { BasicCard, basicCardIconClassName } from "@/components/shared/basic-card"
import { McpIcon } from "@/components/shared/mcp-icon"
import { promptArgCount } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { PromptInfo, ServiceInstance } from "@/lib/api"

export function PromptListCard({
  className,
  instance,
  onSelect,
  prompt,
}: {
  className?: string
  instance: ServiceInstance
  onSelect: () => void
  prompt: PromptInfo
}) {
  const { t } = useI18n()
  const displayName = prompt.title?.trim() || prompt.name
  const description = prompt.description?.trim()
  const argCount = promptArgCount(prompt)
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id

  return (
    <BasicCard
      className={className}
      onClick={onSelect}
      icon={
        <McpIcon
          icons={prompt.icons}
          fallback={<ScrollTextIcon className={basicCardIconClassName} />}
          className={basicCardIconClassName}
        />
      }
      title={displayName}
      subtitle={
        <>
          <BadgeCheckIcon className="size-3.5 shrink-0" />
          <span className="truncate">{instance.service_name}</span>
        </>
      }
      body={
        <>
          <p className="truncate">{description || t("noDescription")}</p>
          <p className="truncate lowercase">{t("paramCount", { count: argCount })}</p>
        </>
      }
      footer={
        <p className="truncate">
          by <span className="text-[#0077c2]">{scopeLabel}</span>
        </p>
      }
    />
  )
}
