/**
 * Typed wrapper over the wasm engine.
 *
 * The Rust side speaks JSON strings in both directions (see `src/api.rs`), so
 * this module owns the parsing and the types. Nothing else in the app should
 * import from 'Poker_VRF' directly.
 */
import init, { deal, verify, transcript_version } from 'Poker_VRF'

export type CardView = { rank: number; suit: number; label: string }
export type RankView = { category: string; tiebreak: number[] }

export type OutcomeView = {
  seed: string
  hands: CardView[][]
  ranks: RankView[]
  winner: number
}

export type Transcript = {
  pubkeys: string[]
  commitments: string[]
  reveals: string[]
  preouts: string[]
  proofs: string[]
  winner: number
}

export type GameView = {
  transcript: Transcript
  outcome: OutcomeView
  transcript_json: string
}

export type VerifyView = {
  ok: boolean
  error: string | null
  outcome: OutcomeView | null
}

let ready: Promise<void> | null = null

/** Idempotent — every entry point awaits this before touching the engine. */
export function initEngine(): Promise<void> {
  ready ??= init().then(() => undefined)
  return ready
}

export async function dealGame(players: number, passphrase: string): Promise<GameView> {
  await initEngine()
  return JSON.parse(deal(players, passphrase))
}

/**
 * Verify a transcript document exactly as a third party would.
 *
 * Rejection is a normal return value, not a throw — the tamper panel depends
 * on rendering the failure rather than catching it.
 */
export async function verifyDocument(document: string): Promise<VerifyView> {
  await initEngine()
  return JSON.parse(verify(document))
}

export async function wireVersion(): Promise<number> {
  await initEngine()
  return transcript_version()
}
