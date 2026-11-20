export const toolDetailSectionGrid =
  "grid grid-cols-1 gap-3 @min-[32rem]:grid-cols-[minmax(0,1.5fr)_minmax(0,8.5fr)]"

/** Doc + playground aside: single column when narrow, two columns once the panel is wide enough. */
export const toolDetailPlaygroundGrid =
  "grid min-h-0 min-w-0 flex-1 grid-cols-1 grid-rows-1 gap-6 overflow-hidden @min-[42rem]:grid-cols-[minmax(0,1fr)_minmax(12rem,18rem)]"
export const toolDetailPlaygroundAside = "hidden @min-[42rem]:flex"
export const toolDetailSectionAside = "flex min-w-0 items-center self-stretch pr-3"
export const toolDetailSectionLabel = "break-words text-sm font-medium"
export const toolDetailSplitSection = "border-b py-4"
