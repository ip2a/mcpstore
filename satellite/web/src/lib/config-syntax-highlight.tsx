import type { ReactNode } from "react"

/** ServerConfig / mcpServers keys we care about in the add-service preview. */
export const CONFIG_FIELD_KEYS = new Set([
  "mcpServers",
  "transport",
  "command",
  "args",
  "url",
  "description",
  "workingDir",
  "env",
  "headers",
  "auth",
  "type",
  "client_id",
  "client_metadata_url",
  "handshakeMode",
])

/** linshi2 / VS Code Dark+ palette on #1d1e22 */
const tokenClass = {
  knownKey: "text-[#9cdcfe]",
  key: "text-[#9cdcfe]/70",
  string: "text-[#b6cea8]",
  number: "text-[#b5cea8]",
  boolean: "text-[#569cd6]",
  null: "text-[#569cd6]",
  punctuation: "text-[#d4d4d4]",
  comment: "text-[#6a9955]",
  bare: "text-white/90",
} as const

type JsonTokenType = keyof typeof tokenClass | "whitespace"

type Token = { type: JsonTokenType; value: string }

function keyClass(raw: string): "knownKey" | "key" {
  const name = raw.replace(/^"|"$/g, "")
  return CONFIG_FIELD_KEYS.has(name) ? "knownKey" : "key"
}

function tokenizeJson(text: string): Token[] {
  const tokens: Token[] = []
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
      tokens.push({ type: isKey ? keyClass(value) : "string", value })
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

function tokenizeToml(text: string): Token[] {
  const tokens: Token[] = []
  let index = 0

  while (index < text.length) {
    const char = text[index]

    if (char === "#") {
      const start = index
      while (index < text.length && text[index] !== "\n") index++
      tokens.push({ type: "comment", value: text.slice(start, index) })
      continue
    }

    if (/\s/.test(char)) {
      const start = index
      while (index < text.length && /\s/.test(text[index])) index++
      tokens.push({ type: "whitespace", value: text.slice(start, index) })
      continue
    }

    if ("[]{},=.".includes(char)) {
      tokens.push({ type: "punctuation", value: char })
      index++
      continue
    }

    if (char === '"' || char === "'") {
      const quote = char
      const start = index
      index++
      while (index < text.length && text[index] !== "\n") {
        if (text[index] === "\\" && quote === '"') {
          index += 2
          continue
        }
        if (text[index] === quote) {
          index++
          break
        }
        index++
      }
      tokens.push({ type: "string", value: text.slice(start, index) })
      continue
    }

    if (char === "-" || /\d/.test(char)) {
      const start = index
      while (index < text.length && /[-+eE.\d]/.test(text[index])) index++
      tokens.push({ type: "number", value: text.slice(start, index) })
      continue
    }

    if (text.startsWith("true", index) && !/\w/.test(text[index + 4] ?? "")) {
      tokens.push({ type: "boolean", value: "true" })
      index += 4
      continue
    }

    if (text.startsWith("false", index) && !/\w/.test(text[index + 5] ?? "")) {
      tokens.push({ type: "boolean", value: "false" })
      index += 5
      continue
    }

    if (/[A-Za-z_]/.test(char)) {
      const start = index
      while (index < text.length && /[A-Za-z0-9_-]/.test(text[index])) index++
      const value = text.slice(start, index)
      let peek = index
      while (peek < text.length && /[ \t]/.test(text[peek])) peek++
      const beforeAssign = text[peek] === "="
      tokens.push({
        type: beforeAssign ? keyClass(value) : "bare",
        value,
      })
      continue
    }

    tokens.push({ type: "punctuation", value: char })
    index++
  }

  return tokens
}

function renderTokens(tokens: Token[]): ReactNode {
  return tokens.map((token, index) => {
    if (token.type === "whitespace") return token.value
    return (
      <span key={`${index}-${token.type}-${token.value.slice(0, 24)}`} className={tokenClass[token.type]}>
        {token.value}
      </span>
    )
  })
}

export function highlightConfigJson(text: string): ReactNode {
  return renderTokens(tokenizeJson(text))
}

export function highlightConfigToml(text: string): ReactNode {
  return renderTokens(tokenizeToml(text))
}

export function highlightServiceConfig(text: string, format: "json" | "toml"): ReactNode {
  return format === "json" ? highlightConfigJson(text) : highlightConfigToml(text)
}
