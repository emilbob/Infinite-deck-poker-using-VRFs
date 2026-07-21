/**
 * A bar reading of the shared seed — one column per byte, height set by that
 * byte's value.
 *
 * Purely a rendering of data already on screen. It earns its place by making
 * "a different seed means a different game" immediately obvious: deal twice
 * and the profile is unrecognisable, which is the property the protocol
 * exists to guarantee. Monochrome acid, because colour here would be
 * decoration rather than information.
 */
export function SeedFingerprint({ seed, cells = 24 }: { seed: string; cells?: number }) {
  const bytes = seed.match(/../g)?.slice(0, cells) ?? []

  return (
    <div
      className="border-line flex h-6 items-end gap-px border-2 px-px pb-px"
      // The seed is printed in full alongside; this is a duplicate reading.
      aria-hidden
      title={seed}
    >
      {bytes.map((b, i) => (
        <span
          key={i}
          className="bg-acid w-full"
          // 15% floor so a zero byte still shows a column rather than a gap.
          style={{ height: `${15 + (parseInt(b, 16) / 255) * 85}%` }}
        />
      ))}
    </div>
  )
}
