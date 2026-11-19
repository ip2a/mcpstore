import { useEffect, useState, type ReactNode } from "react"
import { PencilIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type Props = {
  value: string
  /** Upstream / pre-override value shown on the left. Falls back to `value`. */
  originalValue?: string
  overridden: boolean
  label: string
  multiline?: boolean
  disabled?: boolean
  busy?: boolean
  className?: string
  textClassName?: string
  /** Return `false` to keep the dialog open (e.g. validation failed). */
  onSave: (next: string) => Promise<boolean | void> | boolean | void
  children?: ReactNode
}

const FILTER_DIALOG_SIZE =
  "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none h-[54.4dvh] w-[min(90vw,calc(54.4dvh*1.618))]"

/** Hover-reveal pencil → dialog edit. Overridden values render green. */
export function InlineRouteField({
  value,
  originalValue,
  overridden,
  label,
  multiline = false,
  disabled = false,
  busy = false,
  className,
  textClassName,
  onSave,
  children,
}: Props) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [draft, setDraft] = useState(value)
  const [saving, setSaving] = useState(false)
  const baseline = originalValue ?? value

  useEffect(() => {
    if (open) setDraft(value)
  }, [open, value])

  async function submit() {
    if (draft === value) {
      setOpen(false)
      return
    }
    setSaving(true)
    try {
      const ok = await onSave(draft)
      if (ok === false) return
      setOpen(false)
    } finally {
      setSaving(false)
    }
  }

  const display = children ?? value
  const editable = !disabled
  const originalText = baseline.trim() || t("noDescription")

  return (
    <>
      <span
        className={cn("group/inline-override inline-flex max-w-full items-start gap-1.5", className)}
      >
        <span
          className={cn(
            "min-w-0 break-words",
            textClassName,
            overridden && "text-emerald-600 dark:text-emerald-400",
          )}
        >
          {display}
        </span>
        {editable ? (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            className="mt-0.5 shrink-0 opacity-0 transition-opacity group-hover/inline-override:opacity-100 focus-visible:opacity-100"
            aria-label={t("edit")}
            title={t("edit")}
            disabled={busy || saving}
            onClick={() => setOpen(true)}
          >
            <PencilIcon />
          </Button>
        ) : null}
      </span>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className={FILTER_DIALOG_SIZE}>
          <DialogHeader className="shrink-0 border-b px-4 py-4 @min-[480px]:px-5">
            <DialogTitle>{label}</DialogTitle>
          </DialogHeader>

          {/* Filter dialog is often ~637px wide — @min-[640px] never hits; use 480px. */}
          <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 overflow-hidden px-4 py-4 @min-[480px]:grid-cols-2 @min-[480px]:gap-5 @min-[480px]:px-5">
            <div className="flex min-h-0 min-w-0 flex-col gap-2 overflow-hidden">
              <Label className="shrink-0 text-muted-foreground">{t("routeOriginalValue")}</Label>
              <div className="min-h-0 flex-1 overflow-auto rounded-md border bg-muted/30 px-3 py-2">
                <p
                  className={cn(
                    "break-words whitespace-pre-wrap text-sm text-muted-foreground",
                    !baseline.trim() && "italic",
                    !multiline && "font-mono",
                  )}
                >
                  {originalText}
                </p>
              </div>
            </div>

            <div className="flex min-h-0 min-w-0 flex-col gap-2 overflow-hidden">
              <Label htmlFor="inline-override-draft" className="shrink-0">
                {t("routeNewValue")}
              </Label>
              {multiline ? (
                <Textarea
                  id="inline-override-draft"
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  className="min-h-0 flex-1 resize-none"
                  autoFocus
                />
              ) : (
                <Input
                  id="inline-override-draft"
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  className="font-mono"
                  autoFocus
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault()
                      void submit()
                    }
                  }}
                />
              )}
            </div>
          </div>

          <div className="shrink-0 border-t px-5 py-3.5">
            <DialogFooter className="sm:justify-end">
              <DialogClose asChild>
                <Button type="button" variant="outline" disabled={saving}>
                  {t("cancel")}
                </Button>
              </DialogClose>
              <Button type="button" onClick={() => void submit()} disabled={saving || busy}>
                {saving ? t("saving") : t("save")}
              </Button>
            </DialogFooter>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
