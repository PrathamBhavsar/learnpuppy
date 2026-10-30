import { create } from "zustand"

type PaletteState = {
  open: boolean
  setOpen: (open: boolean) => void
  toggle: () => void
}

export const usePalette = create<PaletteState>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
  toggle: () => set((s) => ({ open: !s.open })),
}))
