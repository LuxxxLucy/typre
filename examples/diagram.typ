// Self-contained schematic for typre testing. Arbitrary A/B/C labels, no external module.
#import "@preview/cetz:0.4.2"

#let palette = (
  text: rgb("#1e293b"),
  sub: rgb("#64748b"),
  req: rgb("#2563eb"),
  resp: rgb("#dc2626"),
  boundary: rgb("#94a3b8"),
  node-fill: rgb("#f8fafc"),
  node-stroke: rgb("#cbd5e1"),
  icon: rgb("#475569"),
  phase2: rgb("#9333ea"),
)

#let node(c, w, h, title, subtitle: none, title-fill: palette.text, sub-fill: palette.sub, text-dx: 0) = {
  import cetz.draw: rect, content
  rect((c.at(0) - w / 2, c.at(1) - h / 2), (c.at(0) + w / 2, c.at(1) + h / 2),
       radius: 3pt, fill: palette.node-fill, stroke: 0.6pt + palette.node-stroke)
  let tx = c.at(0) + text-dx
  if subtitle == none {
    content((tx, c.at(1)), text(10.5pt, weight: "bold", fill: title-fill)[#title])
  } else {
    content((tx, c.at(1) + 0.20), text(10.5pt, weight: "bold", fill: title-fill)[#title])
    content((tx, c.at(1) - 0.24), text(8.6pt, fill: sub-fill)[#subtitle])
  }
}

#let edge(pts, color) = {
  import cetz.draw: line
  line(..pts, stroke: 1.0pt + color, mark: (end: ">", fill: color, scale: 0.8))
}

#let icon-rule(c, s: 1.0) = {
  import cetz.draw: line
  let (x, y) = c
  let st = 0.9pt + palette.icon
  line((x - 0.22 * s, y + 0.11 * s), (x + 0.02 * s, y + 0.11 * s), stroke: st)
  line((x + 0.10 * s, y + 0.11 * s), (x + 0.16 * s, y + 0.05 * s), stroke: st)
  line((x + 0.16 * s, y + 0.05 * s), (x + 0.26 * s, y + 0.19 * s), stroke: st)
  line((x - 0.22 * s, y - 0.13 * s), (x + 0.02 * s, y - 0.13 * s), stroke: st)
  line((x + 0.10 * s, y - 0.06 * s), (x + 0.24 * s, y - 0.20 * s), stroke: st)
  line((x + 0.10 * s, y - 0.20 * s), (x + 0.24 * s, y - 0.06 * s), stroke: st)
}

#let icon-model(c, s: 1.0) = {
  import cetz.draw: line, circle
  let (x, y) = c
  let st = 0.8pt + palette.icon
  let n = ((x, y + 0.16 * s), (x - 0.20 * s, y - 0.10 * s), (x + 0.20 * s, y - 0.10 * s), (x, y - 0.02 * s))
  for (a, b) in ((0, 1), (0, 2), (0, 3), (1, 3), (2, 3), (1, 2)) {
    line(n.at(a), n.at(b), stroke: st)
  }
  for p in n {
    circle(p, radius: 0.055 * s, fill: white, stroke: st)
  }
}

#let box-w = 4.4
#let box-h = 1.15
#let left-x = 2.5
#let right-x = 9.5
#let row-y = (-5.8, -7.55, -9.3, -11.05)

#let diagram = {
  cetz.canvas({
    import cetz.draw: *
    set-style(stroke: 0.6pt)

    let stage(c, title, subtitle, icons: ("model",), phase2: false, tag: none) = {
      let tf = if phase2 { palette.phase2 } else { palette.text }
      let sf = if phase2 { palette.phase2 } else { palette.sub }
      node(c, box-w, box-h, title, subtitle: subtitle, title-fill: tf, sub-fill: sf, text-dx: 0.32)
      if tag != none {
        let hc = if tag == 1 { palette.req } else if tag == 2 { palette.resp } else { rgb("#c026d3") }
        content((c.at(0) + box-w / 2 + 0.28, c.at(1)), anchor: "west",
          box(fill: hc, inset: (x: 2.5pt, y: 1.8pt), radius: 5pt)[#text(6.2pt, fill: white, weight: "bold")[tag #tag]])
      }
      let ix = c.at(0) - box-w / 2 + 0.45
      if icons.len() == 2 {
        icon-rule((ix, c.at(1) + 0.27))
        icon-model((ix, c.at(1) - 0.27))
      } else if icons.at(0) == "rule" {
        icon-rule((ix, c.at(1)))
      } else {
        icon-model((ix, c.at(1)))
      }
    }

    let edge-label(c, s, fill: palette.sub) = content(c, text(8.6pt, fill: fill)[#s])

    // Top row
    node((0.8, 0), 2.6, 1.0, [A])
    node((6, 0), 3.6, 1.1, [B], subtitle: [router])
    node((11.2, 0), 2.6, 1.0, [C])

    edge(((2.1, 0.18), (4.2, 0.18)), palette.req)
    edge(((4.2, -0.18), (2.1, -0.18)), palette.resp)
    edge-label((3.15, 0.55), [io])
    edge(((7.8, 0.18), (9.9, 0.18)), palette.req)
    edge(((9.9, -0.18), (7.8, -0.18)), palette.resp)
    edge-label((8.85, 0.55), [io])

    // B <-> Hub: two double-headed arrows.
    line((5.6, -0.55), (5.6, -3.4),
         stroke: 1.0pt + palette.req, mark: (start: ">", end: ">", fill: palette.req, scale: 0.8))
    line((6.4, -0.55), (6.4, -3.4),
         stroke: 1.0pt + palette.resp, mark: (start: ">", end: ">", fill: palette.resp, scale: 0.8))
    content((2.55, -1.55), text(8.2pt, fill: palette.sub)[
      #align(center)[channel D \ over socket]
    ])

    // System boundary
    rect((-1.1, -2.6), (13.1, -13.55), radius: 8pt,
         stroke: (paint: palette.boundary, thickness: 0.8pt, dash: "dashed"))
    content((-0.6, -3.15), anchor: "west", text(11.5pt, fill: palette.text)[System X])

    node((6, -3.85), 3.4, 0.9, [Hub])

    content((3.0, -4.85), text(9.5pt, weight: "bold", fill: palette.req)[Path P])
    content((9.0, -4.85), text(9.5pt, weight: "bold", fill: palette.resp)[Path Q])

    // Left chain
    edge(((4.3, -3.85), (1.6, -3.85), (1.6, row-y.at(0) + box-h / 2)), palette.req)
    edge-label((2.9, -3.5), [m])
    edge(((4.45, row-y.at(0) + box-h / 2), (4.45, -4.3)), palette.req)

    stage((left-x, row-y.at(0)), [Stage A], [filter], icons: ("rule", "model"), tag: 1)
    stage((left-x, row-y.at(1)), [Stage B], [check], tag: 3)
    stage((left-x, row-y.at(2)), [Stage C], [verify\*], phase2: true, tag: 3)

    for i in (0, 1) {
      edge(((1.6, row-y.at(i) - box-h / 2), (1.6, row-y.at(i + 1) + box-h / 2)), palette.req)
      edge(((3.4, row-y.at(i + 1) + box-h / 2), (3.4, row-y.at(i) - box-h / 2)), palette.req)
    }

    // Right chain
    edge(((7.7, -3.85), (10.4, -3.85), (10.4, row-y.at(0) + box-h / 2)), palette.resp)
    edge-label((9.1, -3.5), [m])
    edge(((7.55, row-y.at(0) + box-h / 2), (7.55, -4.3)), palette.resp)

    stage((right-x, row-y.at(0)), [Stage D], [authorize], icons: ("rule",), tag: 2)
    stage((right-x, row-y.at(1)), [Stage E], [detect], icons: ("rule", "model"), tag: 2)
    stage((right-x, row-y.at(2)), [Stage F], [classify], tag: 2)
    stage((right-x, row-y.at(3)), [Stage G], [audit\*], phase2: true, tag: 2)

    for i in (0, 1, 2) {
      edge(((10.4, row-y.at(i) - box-h / 2), (10.4, row-y.at(i + 1) + box-h / 2)), palette.resp)
      edge(((8.6, row-y.at(i + 1) + box-h / 2), (8.6, row-y.at(i) - box-h / 2)), palette.resp)
    }

    node((6, -12.7), 11.4, 1.1, [Log], subtitle: [observes every stage])

    // Legend
    let leg-txt(x, y, s) = content((x, y), anchor: "west", text(9pt, fill: palette.text)[#s])
    let leg-badge(x, y, n, col) = content((x, y), anchor: "west",
      box(fill: col, inset: (x: 2.5pt, y: 1.8pt), radius: 5pt)[#text(6.2pt, fill: white, weight: "bold")[tag #n]])

    edge(((2.15, -14.3), (2.6, -14.3)), palette.req)
    leg-txt(2.75, -14.3, [Request])
    edge(((2.6, -14.9), (2.15, -14.9)), palette.resp)
    leg-txt(2.75, -14.9, [Response])

    icon-rule((4.8, -14.3))
    leg-txt(5.4, -14.3, [Rule based])
    icon-model((4.8, -14.9))
    leg-txt(5.4, -14.9, [Model based])
    content((4.8, -15.5), text(12pt, weight: "bold", fill: palette.phase2)[✱])
    leg-txt(5.4, -15.5, [Phase 2])

    leg-badge(7.9, -14.3, 1, palette.req)
    leg-txt(9.15, -14.3, [input tag])
    leg-badge(7.9, -14.9, 2, palette.resp)
    leg-txt(9.15, -14.9, [output tag])
    leg-badge(7.9, -15.5, 3, rgb("#c026d3"))
    leg-txt(9.15, -15.5, [side-channel tag])
  })
}

#set page(width: auto, height: auto, margin: 10pt, fill: none)
#diagram
