import { useRef } from 'react'
import { useGSAP } from '@gsap/react'
import gsap from 'gsap'
import type { OutcomeView } from '../engine'
import { PlayingCard } from './PlayingCard'

/** Split a hand category like "FullHouse" into "Full House". */
const humanise = (category: string) => category.replace(/([a-z])([A-Z])/g, '$1 $2')

export function Table({ outcome }: { outcome: OutcomeView }) {
  const scope = useRef<HTMLDivElement>(null)

  // Re-deal animation. Keyed on the seed so a new game replays it.
  useGSAP(
    () => {
      gsap.from('.card', {
        opacity: 0,
        y: -24,
        rotateZ: () => gsap.utils.random(-8, 8),
        duration: 0.4,
        ease: 'power2.out',
        stagger: 0.035,
      })
    },
    { scope, dependencies: [outcome.seed] },
  )

  return (
    <div ref={scope} className="flex flex-col gap-3">
      {outcome.hands.map((hand, seat) => {
        const won = seat === outcome.winner
        return (
          <div
            key={seat}
            className={`flex flex-wrap items-center gap-3 rounded-xl border p-3 transition-colors ${
              won ? 'border-gold/60 bg-gold/5' : 'border-edge/60 bg-felt-900/40'
            }`}
          >
            <div className="w-24 shrink-0">
              <div className="text-sm font-medium">{seat === 0 ? 'You' : `Player ${seat + 1}`}</div>
              {won && <div className="text-gold text-xs tracking-wide uppercase">winner</div>}
            </div>

            <div className="flex gap-2">
              {hand.map((card, i) => (
                <PlayingCard key={i} card={card} />
              ))}
            </div>

            <div className="ml-auto text-right text-sm text-white/70">
              {humanise(outcome.ranks[seat].category)}
            </div>
          </div>
        )
      })}
    </div>
  )
}
