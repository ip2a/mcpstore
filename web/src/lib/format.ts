export function formatDateTime(value: number | string | null | undefined) {
  if (value === null || value === undefined || value === "") return "-"
  const date = typeof value === "number" ? new Date(value > 100_000_000_000 ? value : value * 1000) : new Date(value)
  if (Number.isNaN(date.getTime())) return "-"
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date)
}
