import { useState, type ComponentProps, type ReactNode } from "react"
import { CheckIcon, ClipboardIcon } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { TypographyInlineCode } from "@/components/ui/typography"
import { useI18n } from "@/lib/i18n-context"
import { highlightJson } from "@/lib/json-syntax-highlight"
import { cn } from "@/lib/utils"

type CodeBlockVariant = "request" | "response"

const variantStyles: Record<CodeBlockVariant, { root: string; header: string; body: string; code: string }> = {
  request: {
    root: "border-zinc-800 bg-zinc-950 text-zinc-50",
    header: "border-zinc-800 bg-zinc-950/90 text-zinc-300",
    body: "bg-zinc-950",
    code: "text-xs text-zinc-100",
  },
  response: {
    root: "border-border bg-muted/30 text-foreground",
    header: "border-border bg-muted/50 text-muted-foreground",
    body: "bg-muted/20",
    code: "text-sm text-foreground",
  },
}

export function CodeBlock({
  variant = "response",
  className,
  children,
  ...props
}: ComponentProps<"div"> & { variant?: CodeBlockVariant }) {
  const styles = variantStyles[variant]

  return (
    <div
      data-slot="code-block"
      className={cn("overflow-hidden rounded-lg border shadow-sm", styles.root, className)}
      {...props}
    >
      {children}
    </div>
  )
}

export function CodeBlockHeader({
  variant = "response",
  className,
  children,
  ...props
}: ComponentProps<"div"> & { variant?: CodeBlockVariant }) {
  const styles = variantStyles[variant]

  return (
    <div
      data-slot="code-block-header"
      className={cn("flex items-center justify-between gap-3 border-b px-3 py-2 text-xs", styles.header, className)}
      {...props}
    >
      {children}
    </div>
  )
}

export function CodeBlockTitle({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("flex min-w-0 items-center gap-2 font-mono", className)} {...props} />
}

export function CodeBlockMethod({ className, ...props }: ComponentProps<"span">) {
  return <TypographyInlineCode className={cn("bg-primary/15 text-primary", className)} {...props} />
}

export function CodeBlockActions({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("flex shrink-0 items-center gap-2", className)} {...props} />
}

export function CodeBlockBody({
  variant = "response",
  className,
  children,
  maxHeight = "14rem",
  fill = false,
  ...props
}: ComponentProps<"div"> & { variant?: CodeBlockVariant; maxHeight?: string; fill?: boolean }) {
  const styles = variantStyles[variant]

  return (
    <div
      data-slot="code-block-body"
      className={cn("relative", fill && "flex min-h-0 flex-1 flex-col", styles.body, className)}
      {...props}
    >
      <ScrollArea
        style={fill ? undefined : { maxHeight }}
        className={cn("w-full", fill && "min-h-0 flex-1")}
      >
        <pre className={cn("overflow-x-auto p-4 font-mono leading-relaxed whitespace-pre-wrap", styles.code)}>
          {children}
        </pre>
      </ScrollArea>
    </div>
  )
}

export function CodeBlockFooter({
  variant = "response",
  className,
  children,
  ...props
}: ComponentProps<"div"> & { variant?: CodeBlockVariant }) {
  const styles = variantStyles[variant]

  return (
    <div
      data-slot="code-block-footer"
      className={cn("flex items-center justify-between gap-3 border-t px-3 py-2 text-xs", styles.header, className)}
      {...props}
    >
      {children}
    </div>
  )
}

export function CodeBlockCopyButton({ value, className }: { value: string; className?: string }) {
  const { t } = useI18n()
  const [copied, setCopied] = useState(false)

  async function onCopy() {
    await navigator.clipboard.writeText(value)
    setCopied(true)
    toast.success(t("copied"))
    window.setTimeout(() => setCopied(false), 1500)
  }

  return (
    <Button
      type="button"
      size="sm"
      variant={copied ? "secondary" : "ghost"}
      className={cn("h-7 px-2 text-xs", className)}
      onClick={() => void onCopy()}
    >
      {copied ? <CheckIcon data-icon="inline-start" /> : <ClipboardIcon data-icon="inline-start" />}
      {t("copy")}
    </Button>
  )
}

export function CodeBlockString({ lines }: { lines: string[] }) {
  return (
    <>
      {lines.map((line, index) => (
        <span key={`${index}-${line}`} className="block">
          {line || "\u00a0"}
        </span>
      ))}
    </>
  )
}

export function CodeBlockJson({ text }: { text: string }) {
  return <>{highlightJson(text)}</>
}

export function CodeBlockPlaceholder({ children }: { children: ReactNode }) {
  return <span className="text-muted-foreground/80 italic">{children}</span>
}
