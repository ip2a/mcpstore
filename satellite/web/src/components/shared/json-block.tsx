import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

export function JsonBlock({ value, className }: { value: unknown; className?: string }) {
  return (
    <ScrollArea className={cn("max-h-96 rounded-md bg-muted", className)}>
      <pre className="p-4 text-sm">{JSON.stringify(value, null, 2)}</pre>
    </ScrollArea>
  )
}
