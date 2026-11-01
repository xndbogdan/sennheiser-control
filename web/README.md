# Sennheiser Control (web)

The browser version of Sennheiser Control: noise control, EQ and headset settings for the MOMENTUM 4 (plus the
experimental models of the desktop app), straight over Bluetooth via Web Serial. Chrome or Edge on desktop; there's a
demo mode with a simulated headset for every other browser.

Everything runs in the browser. The page is static; no data leaves it.

## How it fits together

- `crates/headset` (Rust) is the shared core: headset state, which reads to do after connecting, which GAIA writes
  each UI action needs, and the simulated MOMENTUM 4. The desktop app uses it directly.
- `crates/web` wraps it for the browser with `wasm-bindgen`. Its build output (JS glue, `.d.ts` types and the
  `.wasm`) lives in `lib/core/` and is committed, because Vercel builds have no Rust toolchain.
- `lib/link.ts` moves bytes (Web Serial over Bluetooth RFCOMM, or the simulator). `lib/session.ts` keeps one request
  in flight at a time, times them out and hands every byte to the core. `components/` is the UI.

## Develop

```
pnpm install
pnpm dev          # http://localhost:3000
pnpm core         # rebuild lib/core after changing crates/headset, crates/web or crates/protocol
pnpm lint && pnpm build
```

`pnpm core` needs the Rust toolchain from the repo root (it installs the `wasm32-unknown-unknown` target) and
`wasm-pack` (`cargo install wasm-pack`). Commit `lib/core/` together with the Rust change.

## Deploy

On Vercel, import the repository and set **Root Directory** to `web`. The framework preset (Next.js) and the default
build command work as they are.
