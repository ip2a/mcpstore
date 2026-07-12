"use client"

import * as React from "react"
import { ArrowLeftIcon, MoreHorizontalIcon, PlusIcon, SettingsIcon } from "lucide-react"

import { useNavItems, type AppView } from "@/app/app-view"
import { Button } from "@/components/ui/button"
import { ButtonGroup } from "@/components/ui/button-group"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { AddServiceDialog } from "@/features/services/add-service-dialog"
import { useI18n } from "@/lib/i18n-context"
import { type AgentItem } from "@/lib/api"

const NAV_GAP_PX = 8

type HeaderNavEntry = {
  id: string
  collapsePriority: number
  renderButton: () => React.ReactNode
  renderMenuItem: () => React.ReactNode
}

function NavOverflowMenuContent({ entries }: { entries: HeaderNavEntry[] }) {
  return (
    <DropdownMenuContent align="end" className="w-48">
      <DropdownMenuGroup>
        {entries.map((entry) => (
          <React.Fragment key={entry.id}>{entry.renderMenuItem()}</React.Fragment>
        ))}
      </DropdownMenuGroup>
    </DropdownMenuContent>
  )
}

function NavOverflowMenu({
  ariaLabel,
  entries,
}: {
  ariaLabel: string
  entries: HeaderNavEntry[]
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="icon-sm" aria-label={ariaLabel}>
          <MoreHorizontalIcon />
        </Button>
      </DropdownMenuTrigger>
      <NavOverflowMenuContent entries={entries} />
    </DropdownMenu>
  )
}

function NavOverflowSplitButton({
  ariaLabel,
  entries,
  trailingButton,
}: {
  ariaLabel: string
  entries: HeaderNavEntry[]
  trailingButton: React.ReactNode
}) {
  return (
    <DropdownMenu>
      <ButtonGroup>
        {trailingButton}
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="icon-sm" aria-label={ariaLabel}>
            <MoreHorizontalIcon />
          </Button>
        </DropdownMenuTrigger>
      </ButtonGroup>
      <NavOverflowMenuContent entries={entries} />
    </DropdownMenu>
  )
}

function useHiddenNavIds(
  containerRef: React.RefObject<HTMLElement | null>,
  measureRef: React.RefObject<HTMLElement | null>,
  entries: HeaderNavEntry[],
) {
  const [hiddenIds, setHiddenIds] = React.useState<Set<string>>(() => new Set())
  const entryKey = entries.map((entry) => entry.id).join("|")

  React.useLayoutEffect(() => {
    const container = containerRef.current
    const measure = measureRef.current
    if (!container || !measure) return

    const measureAndUpdate = () => {
      const measureChildren = Array.from(measure.children) as HTMLElement[]
      if (measureChildren.length < entries.length + 1) return

      const itemElements = measureChildren.slice(0, entries.length)
      const moreButton = measureChildren.at(-1)
      if (!moreButton) return

      const itemWidths = itemElements.map((element) => element.getBoundingClientRect().width)
      const moreWidth = moreButton.getBoundingClientRect().width
      const availableWidth = container.clientWidth
      const allVisibleWidth =
        itemWidths.reduce((sum, width) => sum + width, 0) +
        Math.max(0, entries.length - 1) * NAV_GAP_PX

      if (allVisibleWidth <= availableWidth) {
        setHiddenIds((current) => (current.size === 0 ? current : new Set()))
        return
      }

      const hidden = new Set<string>()

      const contentWidth = () => {
        const visibleEntries = entries.filter((entry) => !hidden.has(entry.id))
        const visibleCount = visibleEntries.length

        if (hidden.size === 0) {
          return allVisibleWidth
        }

        if (visibleCount === 0) {
          return moreWidth
        }

        const leadingEntries = visibleEntries.slice(0, -1)
        const trailingEntry = visibleEntries.at(-1)
        if (!trailingEntry) return moreWidth

        const trailingIndex = entries.findIndex((entry) => entry.id === trailingEntry.id)
        const trailingGroupWidth = itemWidths[trailingIndex] + moreWidth
        const leadingWidth = leadingEntries.reduce((sum, entry) => {
          const index = entries.findIndex((candidate) => candidate.id === entry.id)
          return sum + itemWidths[index]
        }, 0)

        if (leadingEntries.length === 0) return trailingGroupWidth
        return leadingWidth + leadingEntries.length * NAV_GAP_PX + trailingGroupWidth
      }

      while (contentWidth() > availableWidth && hidden.size < entries.length) {
        let nextHiddenId: string | null = null
        let nextPriority = -1

        for (const entry of entries) {
          if (hidden.has(entry.id) || entry.collapsePriority <= 0) continue
          if (entry.collapsePriority > nextPriority) {
            nextPriority = entry.collapsePriority
            nextHiddenId = entry.id
          }
        }

        if (!nextHiddenId) break
        hidden.add(nextHiddenId)
      }

      setHiddenIds((current) => {
        if (current.size === hidden.size && [...current].every((id) => hidden.has(id))) {
          return current
        }
        return hidden
      })
    }

    measureAndUpdate()

    const observer = new ResizeObserver(measureAndUpdate)
    observer.observe(container)

    return () => observer.disconnect()
  }, [containerRef, measureRef, entryKey, entries])

  return hiddenIds
}

export function AppHeaderNav({
  agents,
  onAdded,
  onBack,
  onOpenSettings,
  onViewChange,
  view,
}: {
  agents: AgentItem[]
  onAdded: () => Promise<void>
  onBack: () => void
  onOpenSettings: () => void
  onViewChange: (view: AppView) => void
  view: AppView
}) {
  const { t } = useI18n()
  const navItems = useNavItems()
  const isHome = view.name === "services"
  const [addDialogOpen, setAddDialogOpen] = React.useState(false)
  const slotRef = React.useRef<HTMLDivElement>(null)
  const measureRef = React.useRef<HTMLDivElement>(null)

  const entries = React.useMemo<HeaderNavEntry[]>(() => {
    const next: HeaderNavEntry[] = []

    if (!isHome) {
      next.push({
        id: "cancel",
        collapsePriority: 0,
        renderButton: () => (
          <Button type="button" variant="outline" size="sm" onClick={onBack}>
            <ArrowLeftIcon data-icon="inline-start" />
            {t("cancel")}
          </Button>
        ),
        renderMenuItem: () => (
          <DropdownMenuItem onSelect={onBack}>
            <ArrowLeftIcon />
            {t("cancel")}
          </DropdownMenuItem>
        ),
      })
    }

    navItems.forEach((item, index) => {
      if (view.name === item.view.name) return
      const Icon = item.icon
      next.push({
        id: `nav-${item.view.name}`,
        collapsePriority: 10 + index,
        renderButton: () => (
          <Button key={item.view.name} variant="outline" size="sm" onClick={() => onViewChange(item.view)}>
            <Icon data-icon="inline-start" />
            {item.label}
          </Button>
        ),
        renderMenuItem: () => (
          <DropdownMenuItem onSelect={() => onViewChange(item.view)}>
            <Icon />
            {item.label}
          </DropdownMenuItem>
        ),
      })
    })

    next.push({
      id: "add-service",
      collapsePriority: 50,
      renderButton: () => (
        <Button variant="outline" size="sm" onClick={() => setAddDialogOpen(true)}>
          <PlusIcon data-icon="inline-start" />
          {t("add")}
        </Button>
      ),
      renderMenuItem: () => (
        <DropdownMenuItem onSelect={() => setAddDialogOpen(true)}>
          <PlusIcon />
          {t("add")}
        </DropdownMenuItem>
      ),
    })

    next.push({
      id: "settings",
      collapsePriority: 60,
      renderButton: () => (
        <Button variant="outline" size="sm" onClick={onOpenSettings}>
          <SettingsIcon data-icon="inline-start" />
          {t("settings")}
        </Button>
      ),
      renderMenuItem: () => (
        <DropdownMenuItem onSelect={onOpenSettings}>
          <SettingsIcon />
          {t("settings")}
        </DropdownMenuItem>
      ),
    })

    return next
  }, [isHome, navItems, onBack, onOpenSettings, onViewChange, t, view.name])

  const hiddenIds = useHiddenNavIds(slotRef, measureRef, entries)
  const visibleEntries = entries.filter((entry) => !hiddenIds.has(entry.id))
  const overflowEntries = entries.filter((entry) => hiddenIds.has(entry.id))
  const hasOverflow = overflowEntries.length > 0
  const leadingVisibleEntries = hasOverflow ? visibleEntries.slice(0, -1) : visibleEntries
  const trailingVisibleEntry = hasOverflow ? visibleEntries.at(-1) : null

  return (
    <div ref={slotRef} className="relative min-w-0 flex-1">
      <div ref={measureRef} aria-hidden className="pointer-events-none invisible absolute flex gap-2">
        {entries.map((entry) => (
          <div key={entry.id}>{entry.renderButton()}</div>
        ))}
        <Button variant="outline" size="icon-sm" tabIndex={-1}>
          <MoreHorizontalIcon />
        </Button>
      </div>

      <nav className="flex w-full flex-nowrap items-center justify-end gap-2 overflow-hidden">
        {leadingVisibleEntries.map((entry) => (
          <React.Fragment key={entry.id}>{entry.renderButton()}</React.Fragment>
        ))}

        {hasOverflow && trailingVisibleEntry ? (
          <NavOverflowSplitButton
            ariaLabel={t("more")}
            entries={overflowEntries}
            trailingButton={trailingVisibleEntry.renderButton()}
          />
        ) : hasOverflow ? (
          <NavOverflowMenu ariaLabel={t("more")} entries={overflowEntries} />
        ) : null}
      </nav>

      <AddServiceDialog
        agents={agents}
        onAdded={onAdded}
        size="sm"
        open={addDialogOpen}
        onOpenChange={setAddDialogOpen}
        showTrigger={false}
      />
    </div>
  )
}
