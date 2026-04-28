/**
 * Generate an SVG path `d` string for a circular arc.
 * Angles in degrees — 0 = top (12 o'clock), increasing clockwise.
 */
export function describeArc(
  cx: number,
  cy: number,
  r: number,
  startDeg: number,
  endDeg: number,
): string {
  const toRad = (d: number) => ((d - 90) * Math.PI) / 180
  const sweep = endDeg - startDeg

  if (sweep >= 360) {
    const mx = cx + r * Math.cos(toRad(0))
    const my = cy + r * Math.sin(toRad(0))
    const bx = cx + r * Math.cos(toRad(180))
    const by = cy + r * Math.sin(toRad(180))
    return `M ${mx} ${my} A ${r} ${r} 0 1 1 ${bx} ${by} A ${r} ${r} 0 1 1 ${mx} ${my}`
  }

  const sRad = toRad(startDeg)
  const eRad = toRad(endDeg)
  const largeArc = sweep > 180 ? 1 : 0

  const sx = cx + r * Math.cos(sRad)
  const sy = cy + r * Math.sin(sRad)
  const ex = cx + r * Math.cos(eRad)
  const ey = cy + r * Math.sin(eRad)

  return `M ${sx} ${sy} A ${r} ${r} 0 ${largeArc} 1 ${ex} ${ey}`
}
