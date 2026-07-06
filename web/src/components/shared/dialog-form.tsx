import type { ComponentProps, ReactNode } from "react"

import { Button } from "@/components/ui/button"
import { DialogFooter } from "@/components/ui/dialog"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"

type DialogFormProps = ComponentProps<"form">

type DialogFormFooterProps = Omit<ComponentProps<typeof DialogFooter>, "children"> & {
  cancelButtonProps?: ComponentProps<typeof Button>
  cancelLabel?: ReactNode
  children?: ReactNode
  onCancel: () => void
  submitButtonProps?: ComponentProps<typeof Button>
  submitDisabled?: boolean
  submitLabel: ReactNode
  submitting?: boolean
}

export function DialogForm({ className, ...props }: DialogFormProps) {
  return <form className={cn("flex flex-col gap-5", className)} {...props} />
}

export function DialogFormFooter({
  cancelButtonProps,
  cancelLabel = "取消",
  children,
  onCancel,
  submitButtonProps,
  submitDisabled = false,
  submitLabel,
  submitting = false,
  ...props
}: DialogFormFooterProps) {
  return (
    <DialogFooter {...props}>
      <Button type="button" variant="outline" onClick={onCancel} {...cancelButtonProps}>
        {cancelLabel}
      </Button>
      <Button type="submit" {...submitButtonProps} disabled={submitDisabled || submitting || submitButtonProps?.disabled}>
        {submitting ? <Spinner data-icon="inline-start" /> : null}
        {submitLabel}
      </Button>
      {children}
    </DialogFooter>
  )
}
