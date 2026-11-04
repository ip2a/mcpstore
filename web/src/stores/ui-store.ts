import { create } from "zustand"

type UiState = {
  cacheDialogOpen: boolean
  homeHeroCollapsed: boolean
  selectedAgentId: string | null
  setCacheDialogOpen: (open: boolean) => void
  setHomeHeroCollapsed: (collapsed: boolean) => void
  setSelectedAgentId: (agentId: string | null) => void
}

export const useUiStore = create<UiState>((set) => ({
  cacheDialogOpen: false,
  homeHeroCollapsed: false,
  selectedAgentId: null,
  setCacheDialogOpen: (cacheDialogOpen) => set({ cacheDialogOpen }),
  setHomeHeroCollapsed: (homeHeroCollapsed) => set({ homeHeroCollapsed }),
  setSelectedAgentId: (selectedAgentId) => set({ selectedAgentId }),
}))
