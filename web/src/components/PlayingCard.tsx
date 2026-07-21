import type { CardView } from '../engine'

/** Suits 1 (♦) and 2 (♥) are the red ones — matches SUITS in lib.rs. */
const isRed = (suit: number) => suit === 1 || suit === 2

/**
 * A real card face: warm paper white, rank in the corners, suit in the middle.
 * Rank and suit are split from the engine's rendered label so the corner and
 * centre can be typeset separately.
 */
export function PlayingCard({ card }: { card: CardView }) {
  const rank = card.label.slice(0, -1)
  const suit = card.label.slice(-1)
  const ink = isRed(card.suit) ? 'text-card-red' : 'text-card-ink'

  return (
    <div
      className={`card bg-card relative flex h-[4.75rem] w-[3.25rem] shrink-0 flex-col
                  justify-between rounded-[5px] px-1.5 py-1 shadow-sm shadow-black/50
                  ring-1 ring-black/25 select-none ${ink}`}
      aria-label={`${rank} of ${['clubs', 'diamonds', 'hearts', 'spades'][card.suit]}`}
    >
      <span className="text-[11px] leading-none font-semibold tracking-tight">{rank}</span>
      <span className="absolute inset-0 flex items-center justify-center text-[19px] leading-none">
        {suit}
      </span>
      <span className="rotate-180 self-end text-[11px] leading-none font-semibold tracking-tight">
        {rank}
      </span>
    </div>
  )
}
