import { BadgeCheckIcon } from "lucide-react"

import { BasicCard, basicCardIconClassName } from "@/components/shared/basic-card"
import { DefaultToolIcon } from "@/components/shared/default-tool-icon"
import { McpIcon } from "@/components/shared/mcp-icon"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ToolInfo } from "@/lib/api"
import { getToolSchema } from "@/lib/tool-info"

export function ToolListCard({
  className,
  instance,
  onSelect,
  tool,
}: {
  className?: string
  instance: ServiceInstance
  onSelect: () => void
  tool: ToolInfo
}) {
  const { t } = useI18n()
  const displayName = tool.title?.trim() || tool.name
  const schema = getToolSchema(tool) as { properties?: Record<string, unknown> }
  const paramCount = Object.keys(schema.properties || {}).length
  const description = tool.description?.trim()
  const scopeLabel =
    instance.scope.type === "store" ? "store" : instance.scope.agent_id

  return (
    <BasicCard
      className={className}
      onClick={onSelect}
      icon={
        <McpIcon
          icons={tool.icons}
          fallback={<DefaultToolIcon className={basicCardIconClassName} />}
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
          <p className="truncate lowercase">{t("paramCount", { count: paramCount })}</p>
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
