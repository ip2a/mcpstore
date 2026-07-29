import { ChevronDownIcon } from "lucide-react"

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export function AgentIdPicker({
  agentIds,
  className,
  disabled,
  id,
  name,
  onChange,
  placeholder,
  required,
  value,
}: {
  agentIds: string[]
  className?: string
  disabled?: boolean
  id?: string
  name?: string
  onChange: (value: string) => void
  placeholder?: string
  required?: boolean
  value: string
}) {
  const { t } = useI18n()
  const suggestions = [...new Set(agentIds.filter(Boolean))]

  return (
    <InputGroup className={cn("min-w-0 flex-1", disabled && "opacity-50", className)}>
      <InputGroupInput
        id={id}
        name={name}
        value={value}
        placeholder={placeholder ?? t("agentIdPlaceholder")}
        disabled={disabled}
        required={required}
        onChange={(event) => onChange(event.target.value)}
      />
      {suggestions.length ? (
        <InputGroupAddon align="inline-end">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <InputGroupButton size="icon-xs" aria-label={t("select")} disabled={disabled}>
                <ChevronDownIcon />
              </InputGroupButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="max-h-60 min-w-48">
              {suggestions.map((agentId) => (
                <DropdownMenuItem key={agentId} onSelect={() => onChange(agentId)}>
                  {agentId}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </InputGroupAddon>
      ) : null}
    </InputGroup>
  )
}
