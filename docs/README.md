# Parametric Stratocaster CAD Generator

A web-based parametric CAD tool for generating laser-cutter and CNC templates for custom Stratocaster-style electric guitars.

**[Live Demo →](https://YOUR_USERNAME.github.io/stratocaster-cad/)**

## Features

- **Fully Parametric** – Adjust scale length, nut width, body dimensions, tremolo spacing, and more
- **Real-time Preview** – See changes instantly in the browser
- **Laser Cutter Ready** – Color-coded SVG output for different operations:
  - 🔴 Red: Cut through
  - 🔵 Blue: Deep pocket/engrave
  - 🟢 Green: Shallow engrave
  - 🔵 Cyan: Reference lines
- **Precision Fret Layout** – Mathematically correct 12-TET fret positions with verification table
- **Export Options** – Individual SVG files, JSON specification, or bundled ZIP
- **No Server Required** – Pure client-side JavaScript, hosts on GitHub Pages

## Generated Templates

| Template | Description |
|----------|-------------|
| Fretboard | 22-fret layout with slots, inlay positions, tapered outline |
| Body (Top) | Neck pocket, pickup routes, tremolo positions, control cavity |
| Body (Back) | Spring cavity, control cavity routing |
| Neck Profiles | Cross-sections at nut and heel with radius arcs |
| Headstock | 6-in-line tuner positions, outline |

## Quick Start

### GitHub Pages Hosting

1. Fork this repository
2. Go to Settings → Pages
3. Set source to "Deploy from branch" → `main` → `/ (root)`
4. Access at `https://YOUR_USERNAME.github.io/stratocaster-cad/`

### Local Development

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/stratocaster-cad.git
cd stratocaster-cad

# Serve locally (Python)
python -m http.server 8000

# Or with Node.js
npx serve
```

Open `http://localhost:8000` in your browser.

## Default Specifications

Based on standard Stratocaster dimensions:

| Parameter | Value | Notes |
|-----------|-------|-------|
| Scale Length | 647.7 mm | 25.5 inches |
| Nut Width | 43.0 mm | Standard Fender |
| Heel Width | 56.0 mm | At body joint |
| Body Thickness | 44.0 mm | ~1.75 inches |
| Tremolo Spacing | 55.56 mm | Modern 2 3/16" (better parts availability) |
| Frets | 22 | Standard modern Strat |
| Fretboard Radius | 241.3 mm | 9.5 inches |

## Tremolo Spacing Options

| Option | Value | Notes |
|--------|-------|-------|
| Vintage | 52.5 mm | 2 1/16" - Original Fender spec |
| Modern | 55.56 mm | 2 3/16" - Better aftermarket support |
| Modern Metric | 56.0 mm | Convenient metric approximation |

## File Structure

```
stratocaster-cad/
├── index.html          # Complete self-contained application
├── README.md           # This file
└── examples/           # Example exports (optional)
    ├── stratocaster_fretboard.svg
    ├── stratocaster_body_top.svg
    └── stratocaster_spec.json
```

## Technical Notes

### Fret Position Calculation

Uses the standard 12-tone equal temperament formula:

```
position = scale_length × (1 - 1/2^(fret/12))
```

All positions rounded to 0.01mm manufacturing precision.

### SVG Specifications

- All dimensions in millimeters
- ViewBox matches physical dimensions for 1:1 printing
- Stroke widths optimized for laser cutter interpretation
- Compatible with LightBurn, RDWorks, LaserGRBL, and similar software

### Browser Compatibility

Tested on modern browsers (Chrome, Firefox, Safari, Edge). Requires JavaScript enabled.

## Manufacturing Workflow

1. **Generate Templates** – Adjust parameters and export SVGs
2. **Import to CAD/CAM** – Open in your laser cutter software
3. **Verify Scale** – Confirm 1:1 scale before cutting
4. **Assign Operations** – Map colors to cut/engrave settings
5. **Cut Templates** – Use MDF or acrylic for routing templates

## Contributing

Contributions welcome! Areas of interest:

- Additional body shapes (Telecaster, Les Paul, etc.)
- DXF export support
- Compound radius fretboard calculations
- Multi-scale/fanned fret support
- Pickup route customization (HSS, HH, etc.)

## License

MIT License - Free for personal and commercial use.

## Related Projects

- [FretFind2D](http://www.ekips.org/tools/guitar/fretfind2d/) - Fretboard calculator
- [Fusion 360 Guitar Templates](https://www.autodesk.com/) - Full 3D CAD approach

---

*Built for makers who want precision without CAD complexity.*
