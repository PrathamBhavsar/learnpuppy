import { useLive } from "@/stores/live"
import type { TurnBuffer } from "@/stores/live"

const EMPTY: TurnBuffer | undefined = undefined

/**
 * The in-flight assistant turn of `chatId`: streamed text/thinking and tool calls since the last
 * `chat.rest`. `undefined` when nothing is streaming; render the server `ClassroomView` then.
 */
export function useStreamingTurn(
  chatId: string | undefined
): TurnBuffer | undefined {
  return useLive((s) => (chatId ? s.turns[chatId] : EMPTY))
}
