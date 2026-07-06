import { SearchIcon } from "lucide-react"

import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"

export function SearchBox({ placeholder, value, onChange }: { placeholder: string; value: string; onChange: (value: string) => void }) {
  return (
    <InputGroup className="min-w-0 flex-1">
      <InputGroupAddon align="inline-start" className="pointer-events-none">
        <SearchIcon aria-hidden="true" />
      </InputGroupAddon>
      <InputGroupInput placeholder={placeholder} value={value} onChange={(event) => onChange(event.target.value)} />
    </InputGroup>
  )
}
