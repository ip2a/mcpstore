import type { KeyboardEvent, ReactNode } from "react"

import { cn } from "@/lib/utils"

/** Media size inside the 32×32 icon slot (slot is owned by BasicCard). */
export const basicCardIconClassName = "size-[30px]"

/** Basic card (基础卡牌): layout-only list card shell. Owns position, size, type, spacing. */
export function BasicCard({
  className,
  icon,
  title,
  subtitle,
  body,
  footer,
  action,
  onClick,
}: {
  className?: string
  icon: ReactNode
  title: ReactNode
  subtitle?: ReactNode
  body?: ReactNode
  footer?: ReactNode
  /** Optional header-right control (e.g. Install button). Clicks do not trigger onClick. */
  action?: ReactNode
  onClick?: () => void
}) {
  const clickable = Boolean(onClick)

  return (
    <div
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
      className={cn(
        "group relative z-[1] flex h-[9.5rem] flex-col overflow-hidden rounded-lg border border-border/50 bg-card px-4 pt-4 pb-3 text-left",
        "shadow-[0_14px_28px_-20px_rgba(59,130,246,0.28)] transition-[border-color,background-color,box-shadow] duration-[400ms]",
        "hover:z-10 hover:border-sky-500/25 hover:bg-sky-50/90 hover:shadow-none dark:hover:bg-sky-950/40",
        "dark:shadow-[0_14px_28px_-20px_rgba(125,211,252,0.18)]",
        clickable && "cursor-pointer",
        className,
      )}
      onClick={onClick}
      onKeyDown={
        onClick
          ? (event: KeyboardEvent<HTMLDivElement>) => {
              if (event.target instanceof Element && event.target.closest("[data-basic-card-action]")) {
                return
              }
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                onClick()
              }
            }
          : undefined
      }
    >
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="flex min-w-0 items-center gap-2">
          <span
            aria-hidden
            className="flex size-8 shrink-0 items-center justify-center text-muted-foreground"
          >
            {icon}
          </span>
          <span className="min-w-0 flex-1">
            <span className="line-clamp-1 font-mono text-sm font-semibold leading-snug">
              {title}
            </span>
            {subtitle ? (
              <span className="mt-1 flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
                {subtitle}
              </span>
            ) : null}
          </span>
          {action ? (
            <span
              data-basic-card-action
              className="shrink-0"
              onClick={(event) => event.stopPropagation()}
              onKeyDown={(event) => event.stopPropagation()}
            >
              {action}
            </span>
          ) : null}
        </div>

        {body ? (
          <div className="mt-4 min-w-0 w-full text-xs leading-5 text-muted-foreground">{body}</div>
        ) : null}

        {footer ? (
          <div className="mt-auto min-w-0 pt-2 text-xs text-muted-foreground">{footer}</div>
        ) : null}
      </div>
    </div>
  )
}
