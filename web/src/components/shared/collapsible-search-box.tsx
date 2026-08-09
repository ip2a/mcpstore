import { SearchIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { SearchBox } from "@/components/shared/search-box"
import { Button } from "@/components/ui/button"

type CollapsibleSearchBoxProps = {
  id?: string
  placeholder: string
  value: string
  onChange: (value: string) => void
}

// 折叠态：图标按钮；展开态：占满剩余空间的搜索输入框
// - 有值时强制展开
// - Escape 清空并收起
// - 失焦且无值时收起
export function CollapsibleSearchBox({ id, placeholder, value, onChange }: CollapsibleSearchBoxProps) {
  const [open, setOpen] = useState(() => value.length > 0)
  const inputRef = useRef<HTMLInputElement>(null)

  // 展开时自动聚焦
  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  // 外部清空 → 保持折叠语义：仅在折叠态下 value 变空时不做额外操作
  // 外部赋值 → 确保展开
  useEffect(() => {
    if (value) setOpen(true)
  }, [value])

  if (!open) {
    return (
      <Button
        variant="outline"
        size="icon"
        className="ml-auto"
        onClick={() => setOpen(true)}
        aria-label={placeholder}
      >
        <SearchIcon />
      </Button>
    )
  }

  return (
    // ml-auto + flex-1：折叠态靠右，展开态吃掉所有剩余空间
    <div className="ml-auto flex min-w-0 flex-1 items-center">
      <SearchBox
        ref={inputRef}
        id={id}
        placeholder={placeholder}
        value={value}
        onChange={onChange}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            onChange("")
            setOpen(false)
          }
        }}
        onBlur={() => {
          if (!value) setOpen(false)
        }}
      />
    </div>
  )
}