import { useCallback, useEffect, useState } from 'react'
import { dealGame, wireVersion, type GameView } from './engine'
import { Table } from './components/Table'
import { TranscriptPanel } from './components/TranscriptPanel'

export default function App() {
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
      // Never swallow this. A failed wasm init used to leave the page looking
      // idle instead of broken, which is much harder to diagnose than an
      // ugly error box.
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setDealing(false)
    }
  }, [seats, passphrase])

  // Deal once on load so the page is never empty.
  useEffect(() => {
    wireVersion().then(setVersion)
    void deal()
    // Intentionally on-mount only: later deals are user-initiated.
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <main className="mx-auto flex max-w-4xl flex-col gap-8 px-5 py-10">
      <header className="flex flex-col gap-2">
        <h1 className="text-3xl font-semibold tracking-tight">
          Infinite-deck poker <span className="text-gold">on VRFs</span>
        </h1>
        <p className="max-w-2xl text-sm leading-relaxed text-white/60">
          The whole engine is Rust compiled to WebAssembly — dealing and verification both run in
          this tab, with no server involved. Your hand is a function of the shared seed and your
          key, so nobody, this page included, can steer it.
        </p>
      </header>

      <section className="border-edge/60 bg-felt-900/40 flex flex-wrap items-end gap-4 rounded-xl border p-4">
        <label className="flex flex-col gap-1">
          <span className="text-xs tracking-wide text-white/50 uppercase">Seats</span>
          <input
            type="number"
            min={2}
            max={10}
            value={seats}
            onChange={(e) => setSeats(Number(e.target.value))}
            className="border-edge bg-felt-950 w-20 rounded-md border px-2 py-1.5 text-sm outline-none focus:border-white/30"
          />
        </label>

        <label className="flex min-w-56 flex-1 flex-col gap-1">
          <span className="text-xs tracking-wide text-white/50 uppercase">
            Your secret contribution
          </span>
          <input
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            placeholder="passphrase — or leave blank for crypto.getRandomValues"
            className="border-edge bg-felt-950 rounded-md border px-2 py-1.5 text-sm outline-none focus:border-white/30"
          />
        </label>

        <button
          onClick={() => void deal()}
          disabled={dealing}
          className="bg-gold hover:bg-gold/90 rounded-md px-5 py-2 text-sm font-medium
                     text-[#241c07] transition-colors disabled:opacity-50"
        >
          {dealing ? 'Dealing…' : 'Deal'}
        </button>
      </section>

      {error && (
        <div className="border-bad/40 bg-bad/10 text-bad rounded-lg border px-4 py-3 text-sm">
          <strong className="font-medium">The engine failed to run.</strong>{' '}
          <span className="font-mono text-xs">{error}</span>
        </div>
      )}

      {!game && !error && <p className="text-sm text-white/40">Loading the engine…</p>}

      {game && (
        <>
          <section className="flex flex-col gap-3">
            <h2 className="text-lg font-medium">The deal</h2>
            <p className="font-mono text-xs break-all text-white/40">seed {game.outcome.seed}</p>
            <Table outcome={game.outcome} />
          </section>

          <TranscriptPanel document={game.transcript_json} />
        </>
      )}

      <footer className="border-edge/60 flex flex-col gap-2 border-t pt-6 text-xs text-white/40">
        <p>
          This is a demo <em>of</em> the protocol, not a fair game under it: one tab holds every
          player's secret, so whoever reveals last could steer the seed. Real fairness needs a
          transport with reveal timeouts.
        </p>
        {version !== null && <p>transcript wire version {version}</p>}
      </footer>
    </main>
  )
}
