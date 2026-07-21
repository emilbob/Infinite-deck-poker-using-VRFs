import type { CardView } from '../engine'

/** Suits 1 (♦) and 2 (♥) are the red ones — matches SUITS in lib.rs. */
const isRed = (suit: number) => suit === 1 || suit === 2

export function PlayingCard({ card }: { card: CardView }) {
  return (
    <div
      className="card flex h-20 w-14 shrink-0 flex-col items-center justify-center rounded-md
                 bg-[#f4f1ea] shadow-lg shadow-black/40 ring-1 ring-black/20"
    >
      <span
        className={`text-xl leading-none font-semibold ${
          isRed(card.suit) ? 'text-[#c0392b]' : 'text-[#1a1a1a]'
        }`}
      >
        {card.label}
      </span>
    </div>
  )
}
