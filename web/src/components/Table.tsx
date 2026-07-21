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
      // Restrained product motion: a short rise and fade, nothing showy.
      gsap.from('.card', {
        opacity: 0,
        y: 8,
        duration: 0.28,
        ease: 'power2.out',
        stagger: 0.025,
      })
    },
    { scope, dependencies: [outcome.seed] },
  )

  return (
    <div ref={scope} className="border-line divide-line divide-y overflow-hidden rounded-lg border">
      {outcome.hands.map((hand, seat) => {
        const won = seat === outcome.winner
        return (
          <div
            key={seat}
            className={`flex flex-wrap items-center gap-x-5 gap-y-3 px-4 py-3.5 ${
              won ? 'bg-ok/[0.06]' : 'bg-panel'
            }`}
          >
            <div className="w-24 shrink-0">
              <div className="text-sm font-medium">{seat === 0 ? 'You' : `Player ${seat + 1}`}</div>
              {won && (
                <div className="text-ok flex items-center gap-1 text-xs">
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

            <div className="flex gap-1.5">
              {hand.map((card, i) => (
                <PlayingCard key={i} card={card} />
              ))}
            </div>

            <div
              className={`ml-auto text-sm ${won ? 'text-ink font-medium' : 'text-muted'}`}
            >
              {humanise(outcome.ranks[seat].category)}
            </div>
          </div>
        )
      })}
    </div>
  )
}
