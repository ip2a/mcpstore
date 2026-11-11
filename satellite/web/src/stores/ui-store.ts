import { create } from "zustand"

type UiState = {
  kvDialogOpen: boolean
  homeHeroCollapsed: boolean
  selectedAgentId: string | null
  setKvDialogOpen: (open: boolean) => void
  setHomeHeroCollapsed: (collapsed: boolean) => void
  setSelectedAgentId: (agentId: string | null) => void
}

export const useUiStore = create<UiState>((set) => ({
  kvDialogOpen: false,
  homeHeroCollapsed: false,
  selectedAgentId: null,
  setKvDialogOpen: (kvDialogOpen) => set({ kvDialogOpen }),
  setHomeHeroCollapsed: (homeHeroCollapsed) => set({ homeHeroCollapsed }),
  setSelectedAgentId: (selectedAgentId) => set({ selectedAgentId }),
}))
