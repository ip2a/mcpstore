import { ScrollArea } from "@/components/ui/scroll-area"

export function JsonBlock({ value }: { value: unknown }) {
  return (
    <ScrollArea className="max-h-96 rounded-md bg-muted">
      <pre className="p-4 text-sm">{JSON.stringify(value, null, 2)}</pre>
    </ScrollArea>
  )
}
