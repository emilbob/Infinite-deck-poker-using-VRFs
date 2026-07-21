import { useEffect, useState } from 'react'
import { verifyDocument, type VerifyView } from '../engine'

type Props = { document: string }

/**
 * Edit the transcript, watch verification react.
 *
 * This is the demo's whole argument made touchable: the same
 * `verify_transcript` a third party would run, executing in the browser on
 * whatever text is in the box.
 */
export function TranscriptPanel({ document }: Props) {
  const [text, setText] = useState(document)
  const [result, setResult] = useState<VerifyView | null>(null)
  const [checking, setChecking] = useState(false)

  // A fresh deal replaces the document.
  useEffect(() => setText(document), [document])

  // Re-verify on every edit, debounced so typing stays smooth.
  useEffect(() => {
    setChecking(true)
    let cancelled = false
    const t = setTimeout(async () => {
      const v = await verifyDocument(text)
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

  /** Apply a mutation to the parsed document, then re-serialise. */
  const mutate = (fn: (doc: Record<string, unknown>) => void) => {
    try {
      const doc = JSON.parse(text)
      fn(doc)
      setText(JSON.stringify(doc, null, 2))
    } catch {
      /* malformed already — the panel is showing why */
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
      const proofs = doc.proofs as string[]
      proofs[0] = 'ee'.repeat(64)
    })

  const reformat = () =>
    mutate(() => {
      /* parse + re-stringify only: proves formatting is not load-bearing */
    })

  return (
    <section className="flex flex-col gap-3">
      <header className="flex flex-wrap items-baseline gap-x-3">
        <h2 className="text-lg font-medium">Transcript</h2>
        <p className="text-sm text-white/50">
          Everything a third party needs. Edit it — verification runs on each keystroke.
        </p>
      </header>

      <div className="flex flex-wrap gap-2">
        <TamperButton onClick={flipReveal}>Flip a byte in a reveal</TamperButton>
        <TamperButton onClick={forgeProof}>Forge a VRF proof</TamperButton>
        <TamperButton onClick={claimWinner}>Claim a different winner</TamperButton>
        <TamperButton onClick={reformat}>Reformat (should still pass)</TamperButton>
        <TamperButton onClick={() => setText(document)}>Reset</TamperButton>
      </div>

      <Status checking={checking} result={result} />

      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        spellCheck={false}
        className="border-edge/60 bg-felt-950/80 h-72 w-full resize-y rounded-lg border p-3
                   font-mono text-xs leading-relaxed text-white/80 outline-none
                   focus:border-white/30"
      />
    </section>
  )
}

function Status({ checking, result }: { checking: boolean; result: VerifyView | null }) {
  if (checking || !result) {
    return <Banner tone="idle">checking…</Banner>
  }
  if (result.ok) {
    const winner = result.outcome!.winner
    return (
      <Banner tone="ok">
        verified — {winner === 0 ? 'you win' : `player ${winner + 1} wins`}, and every commitment,
        proof and hand recomputes
      </Banner>
    )
  }
  return <Banner tone="bad">rejected — {result.error}</Banner>
}

function Banner({ tone, children }: { tone: 'ok' | 'bad' | 'idle'; children: React.ReactNode }) {
  const styles = {
    ok: 'border-ok/40 bg-ok/10 text-ok',
    bad: 'border-bad/40 bg-bad/10 text-bad',
    idle: 'border-white/15 bg-white/5 text-white/50',
  }[tone]
  const mark = { ok: '✔', bad: '✘', idle: '…' }[tone]
  return (
    <div className={`flex items-start gap-2 rounded-lg border px-3 py-2 text-sm ${styles}`}>
      <span aria-hidden>{mark}</span>
      <span>{children}</span>
    </div>
  )
}

function TamperButton({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className="border-edge hover:border-gold/50 hover:text-gold rounded-md border
                 bg-white/5 px-3 py-1.5 text-xs text-white/70 transition-colors"
    >
      {children}
    </button>
  )
}
