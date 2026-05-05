# houtamelo's Age of Mythology: Retold mods

Source for the AoM:R mods I publish on the Age of Mythology Retold mod platform, plus their development history.

## Mods

- [`mod/idle_auto_repair/`](mod/idle_auto_repair/) — **Idle Auto-Repair**. Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost. Covers villagers for all civs and Norse soldier-builders. Player commands always take priority. See the mod's [README](mod/idle_auto_repair/README.md) for full details.

- [`mod/intelligent_auto_scout/`](mod/intelligent_auto_scout/) — **Intelligent Auto-Scout**. Replaces the engine's auto-scout button on land scouts with a frontier-based exploration system that prioritizes coverage near your town center, coordinates across multiple scouts, and routes any herdable a scout passes near to your nearest town center. Oracles keep their vanilla auto-scout behavior. See the mod's [README](mod/intelligent_auto_scout/README.md) for full details.

- [`mod/intelligent_auto_repair_and_scout/`](mod/intelligent_auto_repair_and_scout/) — Combined-deploy variant of the two mods above. Both mods overlay the same `game/ai/human_assist/human_assist.xs` and so cannot be installed as separate local mods at the same time; this folder ships a single unified overlay so you can run both features together.

## Other contents

- `mod/aom_autorepair_test/` — historical proof-of-concept iterations from the auto-repair research; kept for reference, not deployed.
- `docs/` — development notes (proto_mods.xml syntax reference, AI-script-hook viability research) and the design specs / implementation plans that drove each mod under `docs/superpowers/`.
- `extracted/` — extracted reference files from the game install (proto.xml, doxygen, etc.) used during research.

## License

MIT. See `LICENSE`.
