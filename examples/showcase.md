---
title: typre showcase
author: luxxxlucy
---

# typre

A Rust terminal slideshow.
Mainly Markdown syntax, but provide extra ◊ command interfaces.

## Features

**bold**, _italic_, and inline math ◊typst{e^(pi) + 1}

A clickable hyperlink (click to open): [owickstrom.github.io/the-monospace-web](https://owickstrom.github.io/the-monospace-web/)

- Banana
- Paper boat
  - nested item
- Cucumber

1. Goals
1. Motivations
    1. Intrinsic
    1. Extrinsic

```rust
fn main() {
    println!("hello from a code block");
}
```

---

above is a double horizontal rule.

◊details[A collapsible details block]{
Click the summary to expand it.
A second line of detail.
}

## Typst math

Euler's identity ◊typst{e^(i pi) + 1 = 0} sits inline in this sentence, at text height.

◊typst{E = m c^2 + integral_0^1 x^2 dif x}

◊typst{
#import "@preview/cetz:0.4.2"
#cetz.canvas({
  import cetz.draw: *
  circle((0, 0), radius: 1)
  line((-1.5, 0), (1.5, 0))
})
}

## Sized typst

The same fragment, scaled to 70% of the content column:

◊width[70%]{
#import "diagram.typ": diagram; #diagram
}

## Tables and trees

| Name | Dimensions | Position |
|:-----|:-----------|:---------|
| Boboli Obelisk | 1.41m × 1.41m × 4.87m | 43°45'N 11°15'E |
| Pyramid of Khafre | 215m × 215m × 136m | 29°58'N 31°07'E |

◊tree{
/dev/nvme0n1p2
  usr
    local
    bin
  media
  tmp
}

## Grids and figures

◊grid{
1
2
3
4
}

◊figure[Example: Message passing.]{
┌───────┐ ┌───────┐ ┌───────┐
│Actor 1│ │Actor 2│ │Actor 3│
└───┬───┘ └───┬───┘ └───┬───┘
    │         │  msg 1  │
    │         │────────►│
    │  msg 2  │         │
    │────────►│         │
└───────┘ └───────┘ └───────┘
}

## Charts

◊figure[Things I Have]{
    │                      ████ Usable
15  │
    │                      ░░░░ Broken
12  │     ░
 9  │     ░
 6  │  █  ░     ░
 3  │  █  ░  █  ░
 0  └──▀─────▀─────▀────────────
     Socks Jeans Shirts USB
}
