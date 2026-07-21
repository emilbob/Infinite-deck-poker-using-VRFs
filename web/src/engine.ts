/**
 * Typed wrapper over the wasm engine.
 *
 * The Rust side speaks JSON strings in both directions (see `src/api.rs`), so
 * this module owns the parsing and the types. Nothing else in the app should
 * import from 'Poker_VRF' directly.
 */
import init, {
  deal,
  verify,
  transcript_version,
  round_deal,
  round_answer,
  round_reset,
} from 'Poker_VRF'

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

// --- Catch the Cheat -------------------------------------------------------

export type Tier = 'ByEye' | 'ByArithmetic' | 'Impossible'

/**
 * A round before the player answers. Note what is absent: no verdict, no
 * nonce, no cheat. The engine holds those until an answer is submitted, so the
 * commitment below is a real promise rather than something the UI merely
 * declines to display.
 */
export type RoundView = {
  round: number
  total: number
  commitment: string
  transcript_json: string
  outcome: OutcomeView
  score: number
  answered: number
}

export type AnswerView = {
  correct: boolean
  tampered: boolean
  cheat: string
  tier: Tier | null
  explanation: string
  verifier_error: string | null
  nonce: string
  commitment: string
  score: number
  answered: number
  total: number
  finished: boolean
}

export async function roundDeal(players: number): Promise<RoundView> {
  await initEngine()
  return JSON.parse(round_deal(players))
}

/** Returns `null` if no round is pending — the same round cannot be scored twice. */
export async function roundAnswer(guessTampered: boolean): Promise<AnswerView | null> {
  await initEngine()
  return JSON.parse(round_answer(guessTampered))
}

export async function roundReset(): Promise<void> {
  await initEngine()
  round_reset()
}
