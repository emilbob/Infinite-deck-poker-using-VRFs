import { useEffect, useState } from 'react'
import { verifyDocument, type VerifyView } from '../engine'

type Props = { document: string }

const CHECKS = ['Commitments', 'Seed derivation', 'VRF proofs', 'Hands recomputed'] as const

/**
 * Edit the transcript, watch verification react.
 *
 * The demo's argument made touchable: the same `verify_transcript` a third
 * party would run, executing here, on whatever text is in the box.
 */
export function TranscriptPanel({ document }: Props) {
  const [text, setText] = useState(document)
  const [result, setResult] = useState<VerifyView | null>(null)
  const [checking, setChecking] = useState(false)

  useEffect(() => setText(document), [document])

  useEffect(() => {
    setChecking(true)
    let cancelled = false
    const t = setTimeout(async () => {
      let v: VerifyView
      try {
        v = await verifyDocument(text)
      } catch (e) {
        // An engine failure is not a rejected transcript; say which it is.
        v = { ok: false, error: `Engine error: ${e}`, outcome: null }
      }
      if (!cancelled) {
        setResult(v)
        setChecking(false)
      }
    }, 180)
    return () => {
      cancelled = true
      clearTimeout(t)
    }
  }, [text])

  const mutate = (fn: (doc: Record<string, unknown>) => void) => {
    try {
      const doc = JSON.parse(text)
      fn(doc)
      setText(JSON.stringify(doc, null, 2))
    } catch {
      /* already malformed — the status is showing why */
    }
  }

  const flipReveal = () =>
    mutate((doc) => {
      const reveals = doc.reveals as string[]
      const hex = reveals[0]
      reveals[0] = (hex[0] === '0' ? '1' : '0') + hex.slice(1)
    })

  const claimWinner = () =>
    mutate((doc) => {
      const n = (doc.pubkeys as string[]).length
      doc.winner = (((doc.winner as number) ?? 0) + 1) % n
    })

  const forgeProof = () =>
    mutate((doc) => {
      ;(doc.proofs as string[])[0] = 'ee'.repeat(64)
    })

  const reformat = () => mutate(() => {})

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="display text-2xl">Transcript</h2>
        <Status checking={checking} result={result} />
      </div>

      <p className="text-muted max-w-2xl text-sm leading-relaxed">
        Everything a third party needs to check this game. Edit any byte below — verification
        re-runs as you type.
      </p>

      <div className="border-line bg-panel divide-line grid grid-cols-2 divide-x divide-y border-2 sm:grid-cols-4 sm:divide-y-0">
        {CHECKS.map((c) => (
          <Check key={c} label={c} state={checking || !result ? 'idle' : result.ok} />
        ))}
      </div>

      {result && !result.ok && !checking && (
        <p className="border-bad bg-bad/10 text-bad border-2 px-3 py-2 text-sm">
          {result.error}
        </p>
      )}

      <div className="border-acid bg-acid/[0.07] flex flex-col gap-3 border-2 p-4">
        <div>
          <h3 className="display text-xl">Try to break it</h3>
          <p className="text-muted mt-1 text-sm leading-relaxed">
            Each button below forges part of the game. The verifier should reject all of them and
            tell you exactly which check failed — except <em>Reformat</em>, which only reshuffles
            whitespace and must still pass.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Action onClick={flipReveal}>Flip a reveal byte</Action>
          <Action onClick={forgeProof}>Forge a proof</Action>
          <Action onClick={claimWinner}>Claim another winner</Action>
          <Action onClick={reformat}>Reformat</Action>
          <Action onClick={() => setText(document)}>Reset</Action>
        </div>
      </div>

      <details className="group">
        <summary className="text-faint hover:text-muted cursor-pointer text-sm transition-colors">
          Raw document — edit any byte by hand
        </summary>
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          spellCheck={false}
          className="mono border-line bg-panel text-muted focus:border-acid mt-2 h-72 w-full
                     resize-y border-2 p-3 text-[13px] leading-relaxed outline-none"
        />
      </details>
    </section>
  )
}

function Status({ checking, result }: { checking: boolean; result: VerifyView | null }) {
  if (checking || !result) {
    return <Pill tone="idle">Checking…</Pill>
  }
  return result.ok ? <Pill tone="ok">Verified</Pill> : <Pill tone="bad">Rejected</Pill>
}

function Pill({ tone, children }: { tone: 'ok' | 'bad' | 'idle'; children: React.ReactNode }) {
  const styles = {
    ok: 'text-acid bg-acid/10 border-acid',
    bad: 'text-bad bg-bad/10 border-bad',
    idle: 'text-faint bg-raised border-line',
  }[tone]
  return (
    <span
      className={`label inline-flex items-center gap-1.5 border-2 px-2.5 py-1 ${styles}`}
    >
      <span className="h-2 w-2 bg-current" />
      {children}
    </span>
  )
}

function Check({ label, state }: { label: string; state: boolean | 'idle' }) {
  const mark =
    state === 'idle' ? (
      <span className="text-faint">·</span>
    ) : state ? (
      <span className="text-acid">✓</span>
    ) : (
      <span className="text-bad">✕</span>
    )
  return (
    <div className="flex items-center gap-2 px-3 py-2.5">
      <span className="w-3 text-center text-sm">{mark}</span>
      <span className="text-muted text-sm">{label}</span>
    </div>
  )
}

function Action({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className="border-line bg-raised text-muted hover:border-acid hover:text-acid
                 border-2 px-3 py-1.5 text-sm transition-colors"
    >
      {children}
    </button>
  )
}
