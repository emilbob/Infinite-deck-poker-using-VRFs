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

const TIERS: Tier[] = ['ByEye', 'ByArithmetic', 'Impossible']

const TIER_LABEL: Record<Tier, string> = {
  ByEye: 'By eye',
  ByArithmetic: 'By arithmetic',
  Impossible: 'Impossible',
}

const TIER_BLURB: Record<Tier, string> = {
  ByEye: 'The table contradicts the claim — someone is marked the winner holding a worse hand. Just look.',
  ByArithmetic:
    'Nothing looks wrong. Catching it means hashing the numbers yourself and comparing.',
  Impossible: 'A forged cryptographic proof. Nothing you can see will tell you — and that is the point.',
}

/** How many of each tier the player has met, and how many they called right. */
type Tally = Record<Tier, { seen: number; caught: number }>

const EMPTY_TALLY: Tally = {
  ByEye: { seen: 0, caught: 0 },
  ByArithmetic: { seen: 0, caught: 0 },
  Impossible: { seen: 0, caught: 0 },
}

export function CatchTheCheat() {
  const [round, setRound] = useState<RoundView | null>(null)
  const [answer, setAnswer] = useState<AnswerView | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Kept here rather than in the engine: the tiers only exist to explain the
  // result to a human, and every answer already carries its own tier.
  const [tally, setTally] = useState<Tally>(EMPTY_TALLY)

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
      const scored = await roundAnswer(guess)
      // `null` means no round was pending — the engine refuses to score the
      // same round twice. Say so instead of leaving the buttons up, which
      // reads as the click having been dropped.
      if (scored === null) {
        setError('That round was already scored. Deal the next one.')
        return
      }
      setAnswer(scored)
      if (scored.tier) {
        const tier = scored.tier
        setTally((t) => ({
          ...t,
          [tier]: {
            seen: t[tier].seen + 1,
            caught: t[tier].caught + (scored.correct ? 1 : 0),
          },
        }))
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }, [])

  const restart = useCallback(async () => {
    await roundReset()
    setTally(EMPTY_TALLY)
    await next()
  }, [next])

  useEffect(() => {
    void next()
  }, [next])

  // Rendered inline rather than in place of the mode: replacing the whole view
  // removed the only control that could clear the error, so a single transient
  // failure needed a page reload to escape.
  const banner = error && (
    <div className="border-bad bg-bad/10 text-bad flex flex-wrap items-center gap-3 border-2 px-4 py-3 text-sm">
      <span>
        <strong>Something went wrong.</strong> <span className="mono text-xs">{error}</span>
      </span>
      <button
        onClick={() => void next()}
        className="border-bad ml-auto border-2 px-3 py-1 text-sm transition-colors hover:bg-bad/20"
      >
        Deal another round
      </button>
    </div>
  )

  if (!round) {
    return (
      <div className="flex flex-col gap-4">
        {banner}
        {!error && <p className="text-faint text-sm">Dealing…</p>}
      </div>
    )
  }

  const done = answer?.finished === true

  return (
    <section className="flex flex-col gap-5">
      {banner}

      {/* Only until the first answer is in. Without it the mode drops a visitor
          straight into a scoreboard and two buttons, so the ladder — and the
          fact that its top rung is unwinnable by eye — is never set up, and the
          payoff at the end lands on someone who was never told the rules. */}
      {round.answered === 0 && !answer && <Briefing />}

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
          tally={tally}
        />
      )}
    </section>
  )
}

function Briefing() {
  return (
    <div className="border-line bg-panel flex flex-col gap-3 border-2 p-4">
      <h3 className="display text-xl">How this works</h3>
      <p className="text-muted max-w-2xl text-sm leading-relaxed">
        Ten rounds. Each is either an honest deal or a rigged one, and you call it{' '}
        <strong className="font-medium">before</strong> the verifier does. The game has already
        committed to its answer in the hash below, so it cannot change its mind once you have
        guessed — you can re-check that yourself after each round.
      </p>
      <p className="text-muted max-w-2xl text-sm leading-relaxed">
        The cheats come in three kinds, and they are not equally catchable:
      </p>
      <ol className="border-line bg-line grid gap-0.5 border-2 sm:grid-cols-3">
        {TIERS.map((tier, i) => (
          <li key={tier} className="bg-panel flex flex-col gap-1.5 p-3">
            <div className="flex items-center gap-2">
              {/* `text-base` is a COLOR token here (--color-base, near-black),
                  not a font size — it is the black-on-acid idiom. Sizing with
                  it renders invisible text that still typechecks and builds. */}
              <span
                className={`label flex h-5 w-6 items-center justify-center ${
                  tier === 'Impossible' ? 'bg-acid text-base' : 'border-line text-faint border-2'
                }`}
              >
                {i + 1}
              </span>
              <h4 className={`display text-lg ${tier === 'Impossible' ? 'text-acid' : ''}`}>
                {TIER_LABEL[tier]}
              </h4>
            </div>
            <p className="text-muted text-sm leading-relaxed">{TIER_BLURB[tier]}</p>
          </li>
        ))}
      </ol>
      <p className="text-faint max-w-2xl text-sm leading-relaxed">
        Expect to clear the first kind and stall on the third. That is not you being bad at this —
        it is the reason the maths exists. Your score is broken down by kind at the end.
      </p>
    </div>
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
  tally,
}: {
  answer: AnswerView
  onNext: () => void
  onRestart: () => void
  done: boolean
  tally: Tally
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

      {done && <Debrief score={answer.score} total={answer.total} tally={tally} />}

      <div className="flex flex-wrap gap-3">
        {done ? (
          <Verdict onClick={onRestart}>Play again</Verdict>
        ) : (
          <Verdict onClick={onNext}>Next round</Verdict>
        )}
      </div>
    </div>
  )
}

/**
 * The point of the whole mode, delivered as the player's own numbers.
 *
 * A bare "7 of 10" says nothing — it hides *which* cheats got past them, which
 * is the only interesting part. Broken down by tier, the shape of the argument
 * shows up in their own results: the visible ones were easy, and the forged
 * proofs were never catchable by looking.
 */
function Debrief({ score, total, tally }: { score: number; total: number; tally: Tally }) {
  const seen = TIERS.filter((t) => tally[t].seen > 0)
  const impossible = tally.Impossible

  // Built as whole sentences per case rather than spliced fragments: a run can
  // contain one forged proof or several, and stitching in a count produced
  // "The forged proof were not catchable".
  const one = impossible.seen === 1
  const lead = one
    ? 'The forged proof was not catchable by eye.'
    : `The ${impossible.seen} forged proofs were not catchable by eye.`
  const middle =
    impossible.caught === 0
      ? 'Nothing you could have looked at would have told you.'
      : `Calling ${one ? 'it' : `${impossible.caught} of them`} correctly was a coin flip, not observation — there is nothing in a forged proof for a human to see.`
  const tail = one
    ? 'The verifier caught it instantly'
    : 'The verifier caught every one of them instantly'

  return (
    <div className="border-line bg-panel flex flex-col gap-3 border-2 p-4">
      <p className="display text-acid text-2xl">
        Run complete — {score} of {total}
      </p>

      {seen.length > 0 && (
        <>
          <dl className="border-line bg-line grid gap-0.5 border-2 sm:grid-cols-3">
            {seen.map((tier) => {
              const { caught, seen: n } = tally[tier]
              const clean = caught === n
              return (
                <div key={tier} className="bg-panel flex flex-col gap-1 p-3">
                  <dt className="text-faint label">{TIER_LABEL[tier]}</dt>
                  <dd
                    className={`display text-xl ${
                      tier === 'Impossible' ? 'text-bad' : clean ? 'text-acid' : 'text-muted'
                    }`}
                  >
                    {caught} / {n} caught
                  </dd>
                </div>
              )
            })}
          </dl>
          <p className="text-faint text-sm leading-relaxed">
            Honest rounds are not listed here — only rigged rounds have a kind, so these will not
            add up to your score.
          </p>
        </>
      )}

      {impossible.seen > 0 && (
        <p className="text-muted max-w-2xl text-sm leading-relaxed">
          <strong className="text-bad font-medium">{lead}</strong> {middle} {tail}, in this tab, on
          the same numbers you were staring at. That gap is the argument for the cryptography: you
          cannot audit a deal by reading it, and you do not have to.
        </p>
      )}
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
