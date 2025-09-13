# Repository Guidelines

## Project Structure & Module Organization
- `dbc/` — CAN database files (primary source: `bmw_e90.dbc`). Keep all DBC edits here.
- `docs/` — Architecture notes and reference material. Update when message/signal meaning changes.
- `tests/` (optional) — Add parser/consistency checks for DBCs.

## Build, Test, and Development Commands
- Validate DBC with cantools (Python):
  - `pip install cantools`
  - `python - <<'PY'
import cantools; cantools.database.load_file('dbc/bmw_e90.dbc'); print('DBC OK')
PY`
- Dump as JSON for review: `cantools dump dbc/bmw_e90.dbc > out.json`
- Sanity check with canmatrix (alternative): `pip install canmatrix` then `canconvert dbc/bmw_e90.dbc out.xlsx`.

## Coding Style & Naming Conventions
- Messages: UpperCamelCase, concise (e.g., `EngineStatus`).
- Signals: lower_snake_case with units suffix if applicable (e.g., `vehicle_speed_kph`).
- Enums: SCREAMING_SNAKE_CASE values with clear prefixes.
- IDs: Do not renumber existing messages; propose changes via PR with rationale.
- Comments: Document units, scaling, offset, and source (trace, doc, reverse‑engineered).

## Testing Guidelines
- Add tests in `tests/` named `test_*.py`.
- Minimum: a parse test that loads the DBC and asserts no duplicate IDs, names, or DLC mismatches.
- Target: basic coverage over custom validators (if added). Example:
  - `pytest -q` after installing `pytest` and `cantools`.

## Commit & Pull Request Guidelines
- Commit messages: Conventional Commits (e.g., `feat(dbc): add steering_angle signal`, `fix(dbc): correct speed scaling`).
- PRs must include:
  - Clear description and motivation; link related issues or traces.
  - Before/after examples for scaled values and affected signals.
  - Updated `docs/architecture.md` when meanings/assumptions change.
  - Screenshots or snippets from tools (`cantools dump`, traces) when relevant.

## Security & Configuration Tips
- Do not commit raw traces, VINs, keys, or proprietary data. Use sanitized samples.
- Large artifacts (BLF/PCAP/MF4) belong outside the repo; reference them in PRs privately.
- Keep `dbc/` as the single source of truth; regenerate derived files.

## Agent-Specific Instructions
- Follow these conventions across the entire repo. If adding scripts or tests, keep them self‑contained and reference `dbc/bmw_e90.dbc` via relative paths.
