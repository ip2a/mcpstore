import { ArrowRightIcon, LinkIcon, UnlinkIcon } from "lucide-react"

import { cn } from "@/lib/utils"

type ServiceTransitionDirection = "connect" | "disconnect"

export function ServiceTransitionIcon({
  direction = "connect",
  className,
  ...props
}: React.ComponentProps<"span"> & { direction?: ServiceTransitionDirection }) {
  const FromIcon = direction === "connect" ? UnlinkIcon : LinkIcon
  const ToIcon = direction === "connect" ? LinkIcon : UnlinkIcon

  return (
    <span className={cn("inline-flex items-center gap-0.5 text-current", className)} aria-hidden="true" {...props}>
      <FromIcon className="size-4 shrink-0" />
      <ArrowRightIcon className="size-3.5 shrink-0 opacity-80" />
      <ToIcon className="size-4 shrink-0" />
    </span>
  )
}
