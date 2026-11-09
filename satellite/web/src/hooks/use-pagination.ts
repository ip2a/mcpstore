import { useState } from "react"

const PAGE_SIZES = [10, 20, 50, 100] as const
export type PageSize = (typeof PAGE_SIZES)[number]

export function usePagination(initialPageSize: PageSize = 20) {
  const [pageSize, setPageSize] = useState<PageSize>(initialPageSize)
  const [page, setPage] = useState(0)

  function changePageSize(value: string) {
    setPageSize(Number(value) as PageSize)
    setPage(0)
  }

  function nextPage() {
    setPage((prev) => prev + 1)
  }

  function prevPage() {
    setPage((prev) => Math.max(0, prev - 1))
  }

  function resetPage() {
    setPage(0)
  }

  return {
    page,
    pageSize,
    offset: page * pageSize,
    pageSizes: PAGE_SIZES,
    changePageSize,
    nextPage,
    prevPage,
    resetPage,
    setPage,
  }
}

export function getPaginationInfo(offset: number, itemsLength: number, total: number) {
  const from = total === 0 ? 0 : offset + 1
  const to = Math.min(offset + itemsLength, total)
  const pageCount = Math.max(1, Math.ceil(total / (itemsLength || 1)))
  
  return { from, to, pageCount }
}
