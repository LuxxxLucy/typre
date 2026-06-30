// Self-contained schematic for typre testing. Arbitrary A/B/C labels, no external module.
#import "@preview/cetz:0.4.2"

#let palette = (
  text: rgb("#1e293b"),
  sub: rgb("#64748b"),
  req: rgb("#2563eb"),
  resp: rgb("#dc2626"),
  node-fill: rgb("#f8fafc"),
  node-stroke: rgb("#cbd5e1"),
)

#let node(c, w, h, title, subtitle: none) = {
  import cetz.draw: rect, content
  rect((c.at(0) - w / 2, c.at(1) - h / 2), (c.at(0) + w / 2, c.at(1) + h / 2),
       radius: 3pt, fill: palette.node-fill, stroke: 0.6pt + palette.node-stroke)
  if subtitle == none {
    content(c, text(10.5pt, weight: "bold", fill: palette.text)[#title])
  } else {
    content((c.at(0), c.at(1) + 0.20), text(10.5pt, weight: "bold", fill: palette.text)[#title])
    content((c.at(0), c.at(1) - 0.24), text(8.6pt, fill: palette.sub)[#subtitle])
  }
}

#let edge(pts, color) = {
  import cetz.draw: line
  line(..pts, stroke: 1.0pt + color, mark: (end: ">", fill: color, scale: 0.8))
}

#let diagram = {
  cetz.canvas({
    import cetz.draw: *
    set-style(stroke: 0.6pt)

    let edge-label(c, s) = content(c, text(8.6pt, fill: palette.sub)[#s])

    node((0.8, 0), 2.6, 1.0, [A])
    node((6, 0), 3.6, 1.1, [B], subtitle: [router])
    node((11.2, 0), 2.6, 1.0, [C])

    edge(((2.1, 0.18), (4.2, 0.18)), palette.req)
    edge(((4.2, -0.18), (2.1, -0.18)), palette.resp)
    edge-label((3.15, 0.55), [io])
    edge(((7.8, 0.18), (9.9, 0.18)), palette.req)
    edge(((9.9, -0.18), (7.8, -0.18)), palette.resp)
    edge-label((8.85, 0.55), [io])

    // B <-> D: two double-headed arrows over socket
    line((5.6, -0.55), (5.6, -2.2),
         stroke: 1.0pt + palette.req, mark: (start: ">", end: ">", fill: palette.req, scale: 0.8))
    line((6.4, -0.55), (6.4, -2.2),
         stroke: 1.0pt + palette.resp, mark: (start: ">", end: ">", fill: palette.resp, scale: 0.8))
    content((2.55, -1.4), text(8.2pt, fill: palette.sub)[#align(center)[channel D \ over socket]])

    node((6, -2.7), 2.6, 1.0, [D])
  })
}

#set page(width: auto, height: auto, margin: 10pt, fill: none)
#diagram
