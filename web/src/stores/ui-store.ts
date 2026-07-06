import { create } from "zustand"

type UiState = {
  homeHeroCollapsed: boolean
  selectedAgentId: string | null
  setHomeHeroCollapsed: (collapsed: boolean) => void
  setSelectedAgentId: (agentId: string | null) => void
}

export const useUiStore = create<UiState>((set) => ({
  homeHeroCollapsed: false,
  selectedAgentId: null,
  setHomeHeroCollapsed: (homeHeroCollapsed) => set({ homeHeroCollapsed }),
  setSelectedAgentId: (selectedAgentId) => set({ selectedAgentId }),
}))
