import { useCallback, useEffect, useState } from 'react'
import { dealGame, wireVersion, type GameView } from './engine'
import { Table } from './components/Table'
import { TranscriptPanel } from './components/TranscriptPanel'
import { SeedFingerprint } from './components/SeedFingerprint'
import { CatchTheCheat } from './components/CatchTheCheat'

type Mode = 'verify' | 'catch'

export default function App() {
  const [mode, setMode] = useState<Mode>('verify')
  const [game, setGame] = useState<GameView | null>(null)
  const [seats, setSeats] = useState(3)
  const [passphrase, setPassphrase] = useState('')
  const [version, setVersion] = useState<number | null>(null)
  const [dealing, setDealing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const deal = useCallback(async () => {
    setDealing(true)
    setError(null)
    try {
      setGame(await dealGame(seats, passphrase))
    } catch (e) {
      // Never swallow this. A failed wasm init once left the page looking idle
      // instead of broken, which is far harder to diagnose than an error.
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setDealing(false)
    }
  }, [seats, passphrase])

  useEffect(() => {
    wireVersion().then(setVersion)
    void deal()
    // On mount only: later deals are user-initiated.
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      <main className="mx-auto flex max-w-3xl flex-col gap-10 px-5 py-14">
      <header className="flex flex-col">
        <div className="bg-acid text-base px-4 py-3">
          <h1 className="display text-4xl sm:text-6xl">Infinite-deck poker</h1>
          <p className="label mt-1">VRF / SR25519 / RISTRETTO255 / NO SERVER</p>
        </div>
        <p className="text-muted border-line mt-0 max-w-2xl border-2 border-t-0 p-4 text-base leading-relaxed">
          Provably fair dealing built on sr25519 verifiable random functions. The engine is Rust
          compiled to WebAssembly — dealing and verification both run in this tab, with no server
          involved. Your hand is a function of the shared seed and your key, so nobody, this page
          included, can steer it.
        </p>
        <dl className="text-faint label mt-4 flex flex-wrap gap-x-6 gap-y-1">
          <Meta label="Curve" value="Ristretto255 / sr25519" />
          <Meta label="Hash" value="SHA-256" />
          {version !== null && <Meta label="Transcript" value={`v${version}`} />}
        </dl>
      </header>

      <ModeSwitch mode={mode} setMode={setMode} />

      {mode === 'catch' ? (
        <CatchTheCheat />
      ) : (
        <>
      <HowItWorks />

      <section className="border-line bg-panel flex flex-wrap items-end gap-4 border-2 p-4">
        <Field label="Seats" className="w-20">
          <input
            type="number"
            min={2}
            max={10}
            value={seats}
            onChange={(e) => setSeats(Number(e.target.value))}
            className="border-line bg-raised focus:border-acid w-full border-2 px-2.5 py-1.5 text-sm outline-none"
          />
        </Field>

        <Field label="Your entropy" className="min-w-56 flex-1">
          <input
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            placeholder="Passphrase — or leave blank to use the system RNG"
            className="border-line bg-raised placeholder:text-faint focus:border-acid w-full border-2 px-2.5 py-1.5 text-sm outline-none"
          />
        </Field>

        <button
          onClick={() => void deal()}
          disabled={dealing}
          className="bg-acid display text-base px-8 py-2.5 text-lg transition-opacity
                     hover:opacity-80 disabled:opacity-40"
        >
          {dealing ? 'Dealing…' : 'Deal'}
        </button>
      </section>

      {error && (
        <div className="border-bad bg-bad/10 text-bad border-2 px-4 py-3 text-sm">
          <strong className="font-medium">The engine failed to run.</strong>{' '}
          <span className="mono text-sm">{error}</span>
        </div>
      )}

      {!game && !error && <p className="text-faint text-sm">Loading the engine…</p>}

      {game && (
        <>
          <section className="flex flex-col gap-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h2 className="display text-2xl">The deal</h2>
              <div className="flex items-center gap-3">
                <span className="text-faint mono text-xs">
                  {game.outcome.seed.slice(0, 8)}…{game.outcome.seed.slice(-8)}
                </span>
                <div className="w-32">
                  <SeedFingerprint seed={game.outcome.seed} />
                </div>
              </div>
            </div>
            <Table outcome={game.outcome} />
            <p className="text-muted text-sm leading-relaxed">
              Infinite deck: every card is an independent draw, so duplicates are legal and a hand
              like <span className="mono">K♥ K♥ 9♠</span> is not a bug. It also means{' '}
              <strong className="text-muted font-medium">five of a kind</strong> exists, and it
              beats a straight flush.
            </p>
          </section>

          <TranscriptPanel document={game.transcript_json} />
        </>
      )}

        </>
      )}

      <footer className="border-line text-muted flex flex-col gap-3 border-t pt-6 text-sm">
        <p className="max-w-2xl leading-relaxed">
          This is a demonstration <em>of</em> the protocol rather than a fair game under it: one tab
          holds every player&apos;s secret, so whoever reveals last could steer the seed. Real
          fairness needs a network transport with reveal timeouts.
        </p>
        <a
          href="https://github.com/emilbob/Infinite-deck-poker-using-VRFs"
          className="hover:text-ink w-fit underline underline-offset-4 transition-colors"
        >
          Source on GitHub
        </a>
      </footer>
      </main>
    </>
  )
}

/**
 * The page's orientation. Without this, a visitor sees a dealt hand and no
 * indication that the interesting part is trying to break the verifier —
 * which is the only thing here they can actually do.
 */
function HowItWorks() {
  const steps = [
    {
      n: 1,
      title: 'Deal',
      body: 'Every player commits to a secret, then reveals it. The shared seed mixes all of them, so no one player can steer it.',
    },
    {
      n: 2,
      title: 'Read your hand',
      body: 'Your cards are derived from that seed and your key alone. Nothing after the deal can change them — that is what makes the game checkable.',
    },
    {
      n: 3,
      title: 'Try to break it',
      body: 'Tamper with the transcript at the bottom. The verifier runs here, in your browser, and should reject every edit you make.',
    },
  ]

  return (
    <section className="border-line bg-line grid gap-0.5 border-2 sm:grid-cols-3">
      {steps.map((s) => (
        <div key={s.n} className="bg-panel flex flex-col gap-1.5 p-4">
          <div className="flex items-center gap-2">
            <span className="bg-acid text-base label flex h-5 w-6 items-center justify-center">
              {s.n}
            </span>
            <h2 className="display text-lg">{s.title}</h2>
          </div>
          <p className="text-muted text-sm leading-relaxed">{s.body}</p>
        </div>
      ))}
    </section>
  )
}

function Meta({ label, value }: { label: string; value: string }) {
  return (
    <span className="flex gap-1.5">
      <dt>{label}</dt>
      <dd className="text-muted">{value}</dd>
    </span>
  )
}

function Field({
  label,
  className = '',
  children,
}: {
  label: string
  className?: string
  children: React.ReactNode
}) {
  return (
    <label className={`flex flex-col gap-1.5 ${className}`}>
      <span className="text-faint label">{label}</span>
      {children}
    </label>
  )
}

function ModeSwitch({ mode, setMode }: { mode: Mode; setMode: (m: Mode) => void }) {
  const tabs: { id: Mode; label: string; hint: string }[] = [
    { id: 'verify', label: 'Verify', hint: 'Deal a hand and try to break its transcript' },
    { id: 'catch', label: 'Catch the Cheat', hint: 'Ten rounds. Spot the tampered ones' },
  ]
  return (
    <div className="border-line grid gap-px border-2 sm:grid-cols-2">
      {tabs.map((t) => {
        const on = mode === t.id
        return (
          <button
            key={t.id}
            onClick={() => setMode(t.id)}
            className={`flex flex-col gap-1 p-4 text-left transition-colors ${
              on ? 'bg-acid text-black' : 'bg-panel text-muted hover:text-acid'
            }`}
          >
            <span className="display text-xl">{t.label}</span>
            <span className={`text-sm ${on ? 'text-black/70' : 'text-faint'}`}>{t.hint}</span>
          </button>
        )
      })}
    </div>
  )
}
