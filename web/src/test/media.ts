/** Install a controllable matchMedia. Returns a setter that fires change listeners. */
export function mockMatchMedia(initial: Record<string, boolean> = {}) {
  const state = { ...initial }
  const listeners = new Map<string, Set<(e: MediaQueryListEvent) => void>>()
  window.matchMedia = ((query: string) => ({
    get matches() {
      return state[query] ?? false
    },
    media: query,
    onchange: null,
    addEventListener: (_: string, cb: (e: MediaQueryListEvent) => void) => {
      const set = listeners.get(query) ?? new Set()
      set.add(cb)
      listeners.set(query, set)
    },
    removeEventListener: (_: string, cb: (e: MediaQueryListEvent) => void) =>
      listeners.get(query)?.delete(cb),
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as typeof window.matchMedia
  return (query: string, matches: boolean) => {
    state[query] = matches
    listeners
      .get(query)
      ?.forEach((cb) => cb({ matches, media: query } as MediaQueryListEvent))
  }
}
