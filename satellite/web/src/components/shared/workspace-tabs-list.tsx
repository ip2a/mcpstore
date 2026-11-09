"use client"

import { useLayoutEffect, useRef, type ComponentProps } from "react"

import { TabsList } from "@/components/ui/tabs"
import { WORKSPACE_TABS_LIST_CLASSNAME } from "@/components/shared/workspace-toolbar"
import { cn } from "@/lib/utils"

type WorkspaceTabsListProps = ComponentProps<typeof TabsList> & {
  /** Active tab value — scrolls the active trigger into view when it changes. */
  value?: string
}

/**
 * Workspace toolbar tab strip: fills remaining width beside action buttons,
 * scrolls horizontally when services overflow, and nudges the active tab
 * toward the center so neighbors stay visible.
 */
export function WorkspaceTabsList({
  className,
  value,
  variant = "line",
  ...props
}: WorkspaceTabsListProps) {
  const listRef = useRef<HTMLDivElement>(null)

  useLayoutEffect(() => {
    const list = listRef.current
    if (!list) return
    const active = list.querySelector<HTMLElement>('[data-state="active"]')
    if (!active) return
    active.scrollIntoView({ behavior: "smooth", inline: "center", block: "nearest" })
  }, [value])

  return (
    <TabsList
      ref={listRef}
      variant={variant}
      className={cn(WORKSPACE_TABS_LIST_CLASSNAME, className)}
      {...props}
    />
  )
}
