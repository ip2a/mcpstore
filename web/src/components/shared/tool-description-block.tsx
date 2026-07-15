import { extractToolDescriptionDocs, parseToolDescription } from "@/lib/tool-info"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const sectionLabelKeys: Record<string, string> = {
  Args: "descriptionSectionArgs",
  Parameters: "descriptionSectionArgs",
  Returns: "descriptionSectionReturns",
  Return: "descriptionSectionReturns",
  Raises: "descriptionSectionRaises",
}

export function ToolDescriptionBlock({
  className,
  description,
  omitStructuredSections = false,
  showLabel = true,
}: {
  className?: string
  description?: string
  omitStructuredSections?: boolean
  showLabel?: boolean
}) {
  const { t } = useI18n()
  const text = description?.trim() || ""
  const fallback = text || t("noDescription")
  const parsed = extractToolDescriptionDocs(fallback)
  const sections = omitStructuredSections ? parsed.otherSections : parseToolDescription(fallback).sections

  return (
    <div className={cn("text-sm", className)}>
      <div className={cn(showLabel && "grid gap-3 sm:grid-cols-[4.5rem_minmax(0,1fr)]")}>
        {showLabel ? (
          <span className="pt-0.5 text-xs font-medium uppercase tracking-wide text-muted-foreground">{t("description")}</span>
        ) : null}
        <div className="min-w-0 space-y-3">
          <p className="break-words leading-relaxed text-foreground">{parsed.summary || t("noDescription")}</p>
          {sections.map((section, index) => (
            <div key={`${section.label}-${index}`} className="border-t pt-3">
              {section.label ? (
                <p className="mb-1.5 text-xs font-medium text-muted-foreground">
                  {sectionLabelKeys[section.label] ? t(sectionLabelKeys[section.label]) : section.label}
                </p>
              ) : null}
              <p className="break-words whitespace-pre-wrap font-mono text-xs leading-relaxed text-muted-foreground">{section.content}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
