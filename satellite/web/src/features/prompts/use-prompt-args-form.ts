import { useEffect, useState } from "react"

import type { PromptInfo } from "@/lib/api"
import { buildDefaultPromptArgs, parsePromptArgs } from "@/lib/prompt-info"

export function usePromptArgsForm(prompt: PromptInfo | null) {
  const defs = prompt ? parsePromptArgs(prompt) : []
  const [values, setValues] = useState<Record<string, unknown>>({})

  useEffect(() => {
    if (!prompt) {
      setValues({})
      return
    }
    setValues(buildDefaultPromptArgs(parsePromptArgs(prompt)))
  }, [prompt?.name])

  function setField(name: string, value: unknown) {
    setValues((current) => ({ ...current, [name]: value }))
  }

  return { values, setField, setValues, defs }
}
