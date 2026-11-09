import { defaultToolIconUrl } from "@/assets/icons"
import { cn } from "@/lib/utils"

export function DefaultToolIcon({ className }: { className?: string }) {
  return (
    <img src={defaultToolIconUrl} alt="" aria-hidden className={cn("size-5 shrink-0", className)} />
  )
}
