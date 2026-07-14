import type { ReactNode } from "react"

type JsonTokenType = "key" | "string" | "number" | "boolean" | "null" | "punctuation" | "whitespace"

type JsonToken = { type: JsonTokenType; value: string }

const tokenClassNames: Record<JsonTokenType, string> = {
  key: "text-foreground",
  string: "text-[#0077c2]",
  number: "text-[#e64a19]",
  boolean: "text-[#7b1fa2]",
  null: "text-muted-foreground",
  punctuation: "text-[#757575]",
  whitespace: "",
}

function tokenizeJson(text: string): JsonToken[] {
  const tokens: JsonToken[] = []
  let index = 0

  while (index < text.length) {
    const char = text[index]

    if (/\s/.test(char)) {
      const start = index
      while (index < text.length && /\s/.test(text[index])) index++
      tokens.push({ type: "whitespace", value: text.slice(start, index) })
      continue
    }

    if ("{}[],:".includes(char)) {
      tokens.push({ type: "punctuation", value: char })
      index++
      continue
    }

    if (char === '"') {
      const start = index
      index++
      while (index < text.length) {
        if (text[index] === "\\") {
          index += 2
          continue
        }
        if (text[index] === '"') {
          index++
          break
        }
        index++
      }

      const value = text.slice(start, index)
      let peek = index
      while (peek < text.length && /\s/.test(text[peek])) peek++
      const isKey = text[peek] === ":"
      tokens.push({ type: isKey ? "key" : "string", value })
      continue
    }

    if (char === "-" || /\d/.test(char)) {
      const start = index
      while (index < text.length && /[-+eE.\d]/.test(text[index])) index++
      tokens.push({ type: "number", value: text.slice(start, index) })
      continue
    }

    if (text.startsWith("true", index)) {
      tokens.push({ type: "boolean", value: "true" })
      index += 4
      continue
    }

    if (text.startsWith("false", index)) {
      tokens.push({ type: "boolean", value: "false" })
      index += 5
      continue
    }

    if (text.startsWith("null", index)) {
      tokens.push({ type: "null", value: "null" })
      index += 4
      continue
    }

    tokens.push({ type: "punctuation", value: char })
    index++
  }

  return tokens
}

export function highlightJson(text: string): ReactNode {
  const tokens = tokenizeJson(text)

  return tokens.map((token, index) => {
    const className = tokenClassNames[token.type]
    if (!className) return token.value

    return (
      <span key={`${index}-${token.value}`} className={className}>
        {token.value}
      </span>
    )
  })
}
