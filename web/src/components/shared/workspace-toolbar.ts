/**
 * Shared workspace list toolbar chrome — matches Tools page.
 * `py-2` (16) + `TabsList h-9` (36) → 52px content + 1px border ≈ 53px.
 */
export const WORKSPACE_TOOLBAR_CLASSNAME =
  "flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2"

/** Constrained scrollable strip — never covers toolbar action buttons. */
export const WORKSPACE_TABS_LIST_CLASSNAME =
  "h-9 w-auto min-w-0 flex-1 justify-start overflow-x-auto scroll-smooth [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"

/** PanelCard override so the toolbar can own horizontal/vertical chrome. */
export const WORKSPACE_PANEL_CLASSNAME =
  "flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0"

/** Tabs root under a workspace panel — keep default Tabs `gap-2` (do not set gap-0/gap-3). */
export const WORKSPACE_TABS_CLASSNAME = "flex min-h-0 flex-1 flex-col overflow-hidden"

/** Scroll list inset under the toolbar — pairs with Tabs `gap-2` for tools-aligned top spacing. */
export const WORKSPACE_LIST_INSET_CLASSNAME = "px-4 py-2"
