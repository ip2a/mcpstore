import { cn } from "@/lib/utils"

/** Shared Session / Log / KV table — only pin the first two columns; rest size to content. */
export const TRACE_TABLE_CLASSNAME = "w-full"

/** Identity / layer / time — short mono column. */
export const TRACE_COL1_CLASSNAME = "w-[5.5rem] min-w-[5.5rem]"
export const TRACE_COL1_CELL_CLASSNAME = cn(
  TRACE_COL1_CLASSNAME,
  "max-w-[5.5rem] overflow-hidden whitespace-nowrap font-mono text-xs tabular-nums text-ellipsis",
)

/** Key / tags / target — wider mono column. */
export const TRACE_COL2_CLASSNAME = "w-[14rem] min-w-[14rem]"
export const TRACE_COL2_CELL_CLASSNAME = cn(
  TRACE_COL2_CLASSNAME,
  "max-w-[14rem] overflow-hidden whitespace-nowrap font-mono text-xs text-ellipsis",
)

/** Inline badge/chip row — never wrap; clip overflow. */
export const TRACE_COL_TAGS_CLASSNAME =
  "flex min-w-0 max-w-full flex-nowrap items-center gap-1 overflow-hidden"
