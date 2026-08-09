import { SearchIcon } from "lucide-react"
import { forwardRef } from "react"

import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"

type SearchBoxProps = {
  id?: string
  placeholder: string
  value: string
  onChange: (value: string) => void
  onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void
  onBlur?: (event: React.FocusEvent<HTMLInputElement>) => void
}

// 受控搜索输入框，forwardRef 以便父组件聚焦/收起控制
export const SearchBox = forwardRef<HTMLInputElement, SearchBoxProps>(function SearchBox(
  { id, placeholder, value, onChange, onKeyDown, onBlur },
  ref,
) {
  return (
    <InputGroup className="min-w-0 flex-1">
      <InputGroupAddon align="inline-start" className="pointer-events-none">
        <SearchIcon aria-hidden="true" />
      </InputGroupAddon>
      <InputGroupInput
        ref={ref}
        id={id}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        onBlur={onBlur}
      />
    </InputGroup>
  )
})