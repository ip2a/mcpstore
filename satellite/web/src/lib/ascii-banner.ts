import letterM from "@/assets/ascii/letters/M.ascii?raw"
import letterC from "@/assets/ascii/letters/C.ascii?raw"
import letterP from "@/assets/ascii/letters/P.ascii?raw"
import letterS from "@/assets/ascii/letters/S.ascii?raw"
import letterT from "@/assets/ascii/letters/T.ascii?raw"
import letterO from "@/assets/ascii/letters/O.ascii?raw"
import letterR from "@/assets/ascii/letters/R.ascii?raw"
import letterE from "@/assets/ascii/letters/E.ascii?raw"

export const ASCII_BANNER_COLORS = [
  "#7dd3fc",
  "#86efac",
  "#f0abfc",
  "#facc15",
  "#fb7185",
  "#c4b5fd",
] as const

export function randomAsciiBannerColor() {
  return ASCII_BANNER_COLORS[Math.floor(Math.random() * ASCII_BANNER_COLORS.length)]
}

export const ASCII_LETTERS: Record<string, string> = {
  M: letterM.trim(),
  C: letterC.trim(),
  P: letterP.trim(),
  S: letterS.trim(),
  T: letterT.trim(),
  O: letterO.trim(),
  R: letterR.trim(),
  E: letterE.trim(),
}

/** Optical kerning per letter (em). Negative pulls neighbors closer without changing glyphs. */
export const ASCII_LETTER_KERN: Record<string, { start?: number; end?: number }> = {
  M: { end: 0.5 },
  C: { end: -0.06 },
  P: { end: -0.12 },
  S: { end: 0.08 },
  // T stem is inset ~3 cells; pull both sides so gaps match denser letters
  T: { start: -0.28, end: 0.3 },
  O: { end: 0.42 },
  R: { end: -0.06 },
  E: {},
}
