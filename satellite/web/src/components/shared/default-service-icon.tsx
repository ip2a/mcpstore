import { defaultServiceIconUrl } from "@/assets/icons"
import { cn } from "@/lib/utils"

export function DefaultServiceIcon({ className }: { className?: string }) {
  return (
    <img
      src={defaultServiceIconUrl}
      alt=""
      aria-hidden
      className={cn("size-5 shrink-0", className)}
    />
  )
}
