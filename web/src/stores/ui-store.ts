import { create } from "zustand"

type UiState = {
  cacheDialogOpen: boolean
  homeHeroCollapsed: boolean
  settingsDialogOpen: boolean
  selectedAgentId: string | null
  setCacheDialogOpen: (open: boolean) => void
  setHomeHeroCollapsed: (collapsed: boolean) => void
  setSettingsDialogOpen: (open: boolean) => void
  setSelectedAgentId: (agentId: string | null) => void
}

export const useUiStore = create<UiState>((set) => ({
  cacheDialogOpen: false,
  homeHeroCollapsed: false,
  settingsDialogOpen: false,
  selectedAgentId: null,
  setCacheDialogOpen: (cacheDialogOpen) => set({ cacheDialogOpen }),
  setHomeHeroCollapsed: (homeHeroCollapsed) => set({ homeHeroCollapsed }),
  setSettingsDialogOpen: (settingsDialogOpen) => set({ settingsDialogOpen }),
  setSelectedAgentId: (selectedAgentId) => set({ selectedAgentId }),
}))
