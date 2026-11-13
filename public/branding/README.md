# PeliGames artwork

- `peligames-glass.png`: reusable glass emblem, 1262 × 1246 pixels, transparent PNG.
- `peligames-glass.webp`: smaller transparent version for interface use.

Keep the emblem's proportions when placing it. Leave space around its silhouette.
Use the glass artwork for promotional surfaces; `public/peligames.svg` remains
the application logo. Community banners are in `public/addons/banners/`; their
headlines and links are translated interface elements rather than baked-in text.

The Hub composites this same emblem image over each banner background. Color
filters adapt the lighting without redrawing its shape. Banner backgrounds do
not contain a separate generated copy of the emblem.
