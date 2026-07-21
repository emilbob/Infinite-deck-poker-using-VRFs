import { useCallback, useEffect, useState } from 'react'
import {
  roundAnswer,
  roundDeal,
  roundReset,
  type AnswerView,
  type RoundView,
  type Tier,
} from '../engine'
import { Table } from './Table'

const TIER_NOTE: Record<Tier, string> = {
  ByEye: 'Catchable by eye — the table contradicted the claim.',
  ByArithmetic: 'Not visible. Catching it meant hashing the reveal yourself.',
  Impossible: 'Nothing you could see would have told you. Only the proof check catches this.',
}

export function CatchTheCheat() {
  const [round, setRound] = useState<RoundView | null>(null)
  const [answer, setAnswer] = useState<AnswerView | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const next = useCallback(async () => {
    setBusy(true)
    setError(null)
    try {
      setAnswer(null)
      setRound(await roundDeal(3))
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }, [])

  const submit = useCallback(async (guess: boolean) => {
    setBusy(true)
    try {
      setAnswer(await roundAnswer(guess))
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }, [])

  const restart = useCallback(async () => {
    await roundReset()
    await next()
  }, [next])

  useEffect(() => {
    void next()
  }, [next])

  if (error) {
    return (
      <div className="border-bad bg-bad/10 text-bad border-2 px-4 py-3 text-sm">
        <strong>The engine failed to run.</strong> <span className="mono text-xs">{error}</span>
      </div>
    )
  }

  if (!round) return <p className="text-faint text-sm">Dealing…</p>

  const done = answer?.finished === true

  return (
    <section className="flex flex-col gap-5">
      <Scoreboard round={round} answer={answer} />

      <Table outcome={round.outcome} />

      <details>
        <summary className="text-faint hover:text-acid cursor-pointer text-sm transition-colors">
          Transcript — inspect it before you answer
        </summary>
        <textarea
          readOnly
          value={round.transcript_json}
          spellCheck={false}
          className="mono border-line bg-panel text-muted mt-2 h-56 w-full resize-y border-2 p-3
                     text-[13px] leading-relaxed outline-none"
        />
      </details>

      {!answer && (
        <div className="flex flex-col gap-2">
          <p className="text-muted text-sm">Is this round honest, or was it tampered with?</p>
          <div className="flex flex-wrap gap-3">
            <Verdict onClick={() => void submit(false)} disabled={busy}>
              Honest
            </Verdict>
            <Verdict onClick={() => void submit(true)} disabled={busy}>
              Tampered
            </Verdict>
          </div>
        </div>
      )}

      {answer && (
        <Result
          answer={answer}
          onNext={() => void next()}
          onRestart={() => void restart()}
          done={done}
        />
      )}
    </section>
  )
}

function Scoreboard({ round, answer }: { round: RoundView; answer: AnswerView | null }) {
  const answered = answer?.answered ?? round.answered
  const score = answer?.score ?? round.score
  return (
    <div className="border-line grid grid-cols-2 gap-px border-2 sm:grid-cols-3">
      <Cell label="Round">
        {Math.min(round.round + 1, round.total)} / {round.total}
      </Cell>
      <Cell label="Score">
        {score} / {answered}
      </Cell>
      <Cell label="Verdict commitment">
        <span className="mono text-xs">
          {round.commitment.slice(0, 8)}…{round.commitment.slice(-8)}
        </span>
      </Cell>
    </div>
  )
}

function Cell({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="bg-panel flex flex-col gap-1 p-3">
      <span className="text-faint label">{label}</span>
      <span className="display text-xl">{children}</span>
    </div>
  )
}

function Result({
  answer,
  onNext,
  onRestart,
  done,
}: {
  answer: AnswerView
  onNext: () => void
  onRestart: () => void
  done: boolean
}) {
  return (
    <div
      className={`flex flex-col gap-3 border-2 p-4 ${
        answer.correct ? 'border-acid bg-acid/[0.07]' : 'border-bad bg-bad/10'
      }`}
    >
      <div className="flex flex-wrap items-baseline gap-x-3">
        <h3 className={`display text-2xl ${answer.correct ? 'text-acid' : 'text-bad'}`}>
          {answer.correct ? 'Correct' : 'Wrong'}
        </h3>
        <span className="text-muted text-sm">
          {answer.tampered ? 'This round was tampered with' : 'This round was honest'}
          {answer.tampered && ` — ${answer.cheat.toLowerCase()}`}
        </span>
      </div>

      <p className="text-muted text-sm leading-relaxed">{answer.explanation}</p>

      {answer.tier && (
        <p className={`text-sm ${answer.tier === 'Impossible' ? 'text-acid' : 'text-faint'}`}>
          {TIER_NOTE[answer.tier]}
        </p>
      )}

      {/* The claim is never taken on trust: this is the verifier's own words. */}
      <div className="border-line bg-panel border-2 p-3">
        <div className="text-faint label mb-1">Verifier</div>
        <p className="mono text-muted text-xs break-all">
          {answer.verifier_error ?? 'transcript verified — every check passed'}
        </p>
      </div>

      {/* Opening of the commitment shown before the answer, so a suspicious
          player can re-hash it and confirm the game did not move the goalposts. */}
      <div className="border-line bg-panel border-2 p-3">
        <div className="text-faint label mb-1">Commitment opening</div>
        <p className="mono text-muted text-xs break-all">
          H(&quot;poker-vrf.round-verdict.v1&quot; ‖ {answer.tampered ? '01' : '00'} ‖{' '}
          {answer.nonce}) = {answer.commitment}
        </p>
      </div>

      <div className="flex flex-wrap gap-3">
        {done ? (
          <>
            <p className="display text-acid w-full text-2xl">
              Run complete — {answer.score} of {answer.total}
            </p>
            <Verdict onClick={onRestart}>Play again</Verdict>
          </>
        ) : (
          <Verdict onClick={onNext}>Next round</Verdict>
        )}
      </div>
    </div>
  )
}

function Verdict({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void
  disabled?: boolean
  children: React.ReactNode
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="border-acid text-acid hover:bg-acid display border-2 px-8 py-2 text-lg
                 transition-colors hover:text-black disabled:opacity-40"
    >
      {children}
    </button>
  )
}
