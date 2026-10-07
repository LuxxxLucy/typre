#import "@preview/cetz:0.4.2"

#let palette = (
  text: rgb("#1e293b"),
  secondary-text: rgb("#64748b"),
  request: rgb("#2563eb"),
  response: rgb("#dc2626"),
  node-fill: rgb("#f8fafc"),
  node-stroke: rgb("#cbd5e1"),
)

#let draw-node(center, width, height, title, subtitle: none) = {
  import cetz.draw: rect, content
  rect((center.at(0) - width / 2, center.at(1) - height / 2), (center.at(0) + width / 2, center.at(1) + height / 2),
       radius: 3pt, fill: palette.node-fill, stroke: 0.6pt + palette.node-stroke)
  if subtitle == none {
    content(center, text(10.5pt, weight: "bold", fill: palette.text)[#title])
  } else {
    content((center.at(0), center.at(1) + 0.20), text(10.5pt, weight: "bold", fill: palette.text)[#title])
    content((center.at(0), center.at(1) - 0.24), text(8.6pt, fill: palette.secondary-text)[#subtitle])
  }
}

#let draw-arrow(points, color) = {
  import cetz.draw: line
  line(..points, stroke: 1.0pt + color, mark: (end: ">", fill: color, scale: 0.8))
}

#let diagram = {
  cetz.canvas({
    import cetz.draw: *
    set-style(stroke: 0.6pt)

    let draw-edge-label(position, label) = content(position, text(8.6pt, fill: palette.secondary-text)[#label])

    draw-node((0.8, 0), 2.6, 1.0, [A])
    draw-node((6, 0), 3.6, 1.1, [B], subtitle: [router])
    draw-node((11.2, 0), 2.6, 1.0, [C])

    draw-arrow(((2.1, 0.18), (4.2, 0.18)), palette.request)
    draw-arrow(((4.2, -0.18), (2.1, -0.18)), palette.response)
    draw-edge-label((3.15, 0.55), [io])
    draw-arrow(((7.8, 0.18), (9.9, 0.18)), palette.request)
    draw-arrow(((9.9, -0.18), (7.8, -0.18)), palette.response)
    draw-edge-label((8.85, 0.55), [io])

    line((5.6, -0.55), (5.6, -2.2),
         stroke: 1.0pt + palette.request, mark: (start: ">", end: ">", fill: palette.request, scale: 0.8))
    line((6.4, -0.55), (6.4, -2.2),
         stroke: 1.0pt + palette.response, mark: (start: ">", end: ">", fill: palette.response, scale: 0.8))
    content((2.55, -1.4), text(8.2pt, fill: palette.secondary-text)[#align(center)[channel D \ over socket]])

    draw-node((6, -2.7), 2.6, 1.0, [D])
  })
}

#set page(width: auto, height: auto, margin: 10pt, fill: none)
#diagram
