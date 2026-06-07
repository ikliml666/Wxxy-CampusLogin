interface FluidBackgroundProps {
  paused?: boolean
  innerRef?: (el: HTMLDivElement | null) => void
}

export function FluidBackground({ innerRef }: FluidBackgroundProps) {
  return (
    <div
      ref={innerRef}
      className="fixed inset-0 z-0 pointer-events-none"
      style={{
        background: 'var(--surface-main)',
      }}
    />
  )
}
