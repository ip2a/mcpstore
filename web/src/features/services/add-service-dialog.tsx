import { useState, type ComponentProps } from "react"
import { PlusIcon } from "lucide-react"

import { AddServiceForm } from "@/features/services/add-service-form"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { useI18n } from "@/lib/i18n-context"
import { type AgentItem } from "@/lib/api"
import { cn } from "@/lib/utils"

export function AddServiceDialog(props: {
  agents: AgentItem[]
  className?: string
  onAdded: () => Promise<void>
  onOpenChange?: (open: boolean) => void
  open?: boolean
  showTrigger?: boolean
  size?: ComponentProps<typeof Button>["size"]
}) {
  const { t } = useI18n()
  const [internalOpen, setInternalOpen] = useState(false)
  const [session, setSession] = useState(0)
  const open = props.open ?? internalOpen
  const showTrigger = props.showTrigger ?? true

  function onOpenChange(next: boolean) {
    if (props.open === undefined) {
      setInternalOpen(next)
    }
    props.onOpenChange?.(next)
    if (next) setSession((value) => value + 1)
  }

  async function handleAdded() {
    await props.onAdded()
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {showTrigger ? (
        <DialogTrigger asChild>
          <Button variant="outline" size={props.size} className={cn(props.className)}>
            <PlusIcon data-icon="inline-start" />
            {t("add")}
          </Button>
        </DialogTrigger>
      ) : null}
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[78dvh] w-[min(84vw,68rem)]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-3 @min-[640px]:px-5 @min-[640px]:py-3.5">
          <DialogTitle>{t("navAddService")}</DialogTitle>
          <DialogDescription>{t("addServiceDescription")}</DialogDescription>
        </DialogHeader>
        <AddServiceForm
          key={session}
          agents={props.agents}
          onAdded={handleAdded}
          onCancel={() => onOpenChange(false)}
        />
      </DialogContent>
    </Dialog>
  )
}
