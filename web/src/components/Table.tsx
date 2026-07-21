import { useRef } from 'react'
import { useGSAP } from '@gsap/react'
import gsap from 'gsap'
import type { OutcomeView } from '../engine'
import { PlayingCard } from './PlayingCard'

/** "FullHouse" -> "Full house" */
const humanise = (category: string) => {
  const spaced = category.replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase()
  return spaced.charAt(0).toUpperCase() + spaced.slice(1)
}

export function Table({ outcome }: { outcome: OutcomeView }) {
  const scope = useRef<HTMLDivElement>(null)

  useGSAP(
    () => {
      // The flip is decorative; skip the whole timeline when motion is reduced
      // so cards are simply present and face-up.
      if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return

      gsap
        .timeline()
        .from('.card-scene', { opacity: 0, y: 10, duration: 0.2, stagger: 0.03 })
        .from(
          '.card-inner',
          { rotateY: 180, duration: 0.45, ease: 'power2.inOut', stagger: 0.045 },
          '-=0.1',
        )
        .from(
          '.winner-glow',
          { opacity: 0, scaleY: 0.4, duration: 0.4, ease: 'power2.out' },
          '-=0.15',
        )
    },
    { scope, dependencies: [outcome.seed] },
  )

  return (
    <div ref={scope} className="border-line divide-line divide-y border-2">
      {outcome.hands.map((hand, seat) => {
        const won = seat === outcome.winner
        return (
          <div
            key={seat}
            className={`relative flex flex-wrap items-center gap-x-5 gap-y-3 px-4 py-4 ${
              won ? 'bg-acid/[0.06]' : 'bg-panel'
            }`}
          >
            {won && (
              <span className="winner-glow bg-acid absolute inset-y-0 left-0 w-1.5" aria-hidden />
            )}

            <div className="w-24 shrink-0">
              <div className="display text-lg">{seat === 0 ? 'You' : `Player ${String(seat + 1).padStart(2, '0')}`}</div>
              {won && (
                <div className="text-acid label flex items-center gap-1">
                  <svg viewBox="0 0 16 16" className="h-3 w-3" aria-hidden>
                    <path
                      d="M3 8.5l3.2 3.2L13 5"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                  Winner
                </div>
              )}
            </div>

            <div className="flex gap-2">
              {hand.map((card, i) => (
                <PlayingCard key={i} card={card} />
              ))}
            </div>

            <div className={`label ml-auto ${won ? 'text-acid' : 'text-muted'}`}>
              {humanise(outcome.ranks[seat].category)}
            </div>
          </div>
        )
      })}
    </div>
  )
}
