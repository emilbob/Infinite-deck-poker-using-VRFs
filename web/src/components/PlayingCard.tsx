import type { CardView } from '../engine'

/** Suits 1 (♦) and 2 (♥) are the red ones — matches SUITS in lib.rs. */
const isRed = (suit: number) => suit === 1 || suit === 2

const SUIT_NAMES = ['clubs', 'diamonds', 'hearts', 'spades']

/**
 * A real card face on a flippable scene. `.card-inner` is what the deal
 * animation rotates; the reverse face carries the patterned back, so cards
 * turn over rather than fading in.
 */
export function PlayingCard({ card }: { card: CardView }) {
  const rank = card.label.slice(0, -1)
  const suit = card.label.slice(-1)
  const ink = isRed(card.suit) ? 'text-card-red' : 'text-card-ink'

  return (
    <div
      className="card-scene group h-[4.9rem] w-[3.4rem] shrink-0"
      aria-label={`${rank} of ${SUIT_NAMES[card.suit]}`}
    >
      {/* Hover lift lives on its own wrapper: GSAP owns `.card-inner`'s
          transform during the flip, and a CSS transition on the same property
          would fight it every frame. */}
      <div className="h-full w-full transition-transform duration-200 group-hover:-translate-y-1">
        <div className="card-inner">
          {/* Face */}
          <div
            className={`card-face bg-card flex flex-col justify-between px-1.5 py-1
                        border-2 border-black/70 select-none ${ink}`}
          >
            <span className="text-xs leading-none font-semibold tracking-tight">{rank}</span>
            <span className="absolute inset-0 flex items-center justify-center text-[22px] leading-none">
              {suit}
            </span>
            <span className="rotate-180 self-end text-xs leading-none font-semibold tracking-tight">
              {rank}
            </span>
          </div>

          {/* Reverse */}
          <div className="card-face card-face--back" />
        </div>
      </div>
    </div>
  )
}
