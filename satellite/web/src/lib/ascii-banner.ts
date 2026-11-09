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
