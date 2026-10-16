# bevaru × Jackdaw

A [Jackdaw](https://github.com/jbuehler23/jackdaw) project for [bevaru](..):
- **Edit bevaru's lobby:** `assets/lobby.bsn` holds one `LobbyEntry` per card.
- **Reuse bevaru's visual assets:** `PerceptronDiagram` and `LossShapeSurface` work in any scene.

```sh
jd open jackdaw                     # from the bevaru repository root
cd jackdaw && cargo run --release   # run the edited lobby without the editor
```

See [docs/jackdaw.md](../docs/jackdaw.md) for the full guide.
