import type { ComponentProps } from "react"

import { cn } from "@/lib/utils"

export function TypographyH1({ className, ...props }: ComponentProps<"h1">) {
  return <h1 className={cn("scroll-m-20 text-2xl font-semibold tracking-tight", className)} {...props} />
}

export function TypographyH2({ className, ...props }: ComponentProps<"h2">) {
  return <h2 className={cn("scroll-m-20 border-b pb-2 text-lg font-semibold tracking-tight first:mt-0", className)} {...props} />
}

export function TypographyH3({ className, ...props }: ComponentProps<"h3">) {
  return <h3 className={cn("scroll-m-20 text-base font-semibold tracking-tight", className)} {...props} />
}

export function TypographyP({ className, ...props }: ComponentProps<"p">) {
  return <p className={cn("leading-7 text-muted-foreground [&:not(:first-child)]:mt-4", className)} {...props} />
}

export function TypographyLead({ className, ...props }: ComponentProps<"p">) {
  return <p className={cn("text-base text-muted-foreground", className)} {...props} />
}

export function TypographyInlineCode({ className, ...props }: ComponentProps<"code">) {
  return (
    <code
      className={cn("relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm font-semibold", className)}
      {...props}
    />
  )
}

export function TypographyBlockquote({ className, ...props }: ComponentProps<"blockquote">) {
  return <blockquote className={cn("mt-4 border-l-2 pl-4 italic text-muted-foreground", className)} {...props} />
}

export function TypographyList({ className, ...props }: ComponentProps<"ul">) {
  return <ul className={cn("my-4 ml-6 list-disc [&>li]:mt-1", className)} {...props} />
}
