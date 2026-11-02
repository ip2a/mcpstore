import letterM from "@/assets/ascii/letters/M.ascii?raw"
import letterC from "@/assets/ascii/letters/C.ascii?raw"
import letterP from "@/assets/ascii/letters/P.ascii?raw"
import letterS from "@/assets/ascii/letters/S.ascii?raw"
import letterT from "@/assets/ascii/letters/T.ascii?raw"
import letterO from "@/assets/ascii/letters/O.ascii?raw"
import letterR from "@/assets/ascii/letters/R.ascii?raw"
import letterE from "@/assets/ascii/letters/E.ascii?raw"

export const ASCII_BANNER_COLORS = ["#7dd3fc", "#86efac", "#f0abfc", "#facc15", "#fb7185", "#c4b5fd"] as const

export function randomAsciiBannerColor() {
  return ASCII_BANNER_COLORS[Math.floor(Math.random() * ASCII_BANNER_COLORS.length)]
}

function joinLetters(letters: string[], spacing = 2): string {
  const letterLines = letters.map(letter => letter.trim().split('\n'))
  const height = Math.max(...letterLines.map(lines => lines.length))
  const result: string[] = []

  for (let row = 0; row < height; row++) {
    const rowParts: string[] = []
    for (const lines of letterLines) {
      rowParts.push(lines[row] || '')
    }
    result.push(rowParts.join(' '.repeat(spacing)))
  }

  return result.join('\n')
}

export const MCPSTORE_ASCII = joinLetters([
  letterM,
  letterC,
  letterP,
  letterS,
  letterT,
  letterO,
  letterR,
  letterE
])
