import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"

export function OptionToggleGroup<T extends string>({
  className,
  disabled,
  onChange,
  options,
  value,
}: {
  className?: string
  disabled?: boolean
  onChange: (value: T) => void
  options: Array<{ value: T; label: string }>
  value: T
}) {
  return (
    <div
      role="group"
      className={cn("inline-flex flex-wrap rounded-lg border bg-muted/40 p-0.5", className)}
    >
      {options.map((option) => (
        <Button
          key={option.value}
          type="button"
          size="sm"
          variant={value === option.value ? "default" : "ghost"}
          className="h-7 px-3"
          disabled={disabled}
          aria-pressed={value === option.value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </Button>
      ))}
    </div>
  )
}
