import { BadgeCheckIcon, FileStackIcon } from "lucide-react"

import { BasicCard, basicCardIconClassName } from "@/components/shared/basic-card"
import { McpIcon } from "@/components/shared/mcp-icon"
import { resourceMimeType } from "@/features/services/service-panels"
import { useI18n } from "@/lib/i18n-context"
import type { ResourceInfo, ServiceInstance } from "@/lib/api"

export function ResourceListCard({
  className,
  instance,
  onSelect,
  resource,
}: {
  className?: string
  instance: ServiceInstance
  onSelect: () => void
  resource: ResourceInfo
}) {
  const { t } = useI18n()
  const displayName = resource.title?.trim() || resource.name
  const description = resource.description?.trim() || resource.uri
  const mime = resourceMimeType(resource)
  const scopeLabel = instance.scope.type === "store" ? "store" : instance.scope.agent_id

  return (
    <BasicCard
      className={className}
      onClick={onSelect}
      icon={
        <McpIcon
          icons={resource.icons}
          fallback={<FileStackIcon className={basicCardIconClassName} />}
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
          {mime ? (
            <p className="truncate font-mono lowercase">{mime}</p>
          ) : (
            <p className="truncate text-muted-foreground/60">&nbsp;</p>
          )}
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
