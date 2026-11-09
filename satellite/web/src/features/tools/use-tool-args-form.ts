import { useEffect, useState } from "react"

import { buildDefaultArgs, type ToolSchema } from "@/lib/tool-args"
import { getToolSchema } from "@/lib/tool-info"
import type { ToolInfo } from "@/lib/api"

export function useToolArgsForm(tool: ToolInfo | null) {
  const schema: ToolSchema = tool ? (getToolSchema(tool) as ToolSchema) : {}
  const [values, setValues] = useState<Record<string, unknown>>({})

  useEffect(() => {
    if (!tool) {
      setValues({})
      return
    }
    setValues(buildDefaultArgs(schema))
  }, [tool?.name])

  function setField(name: string, value: unknown) {
    setValues((current) => ({ ...current, [name]: value }))
  }

  return { values, setField, setValues, schema }
}
