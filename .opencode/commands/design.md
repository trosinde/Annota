---
description: "Design gate for an OpenSpec change: designer mockups, check, approval request"
---

Run the design gate for OpenSpec change `$ARGUMENTS`:

1. Check `openspec/changes/$ARGUMENTS/` exists and `openspec validate $ARGUMENTS --strict` passes.
2. If `docs/design/kit/` does not exist, first delegate to the `@designer` subagent: "Create the Annota design kit".
3. Delegate to the `@designer` subagent: "Create mockups for change $ARGUMENTS" (workflow in its definition).
4. Look at the returned PNGs yourself. If anything looks off, send the designer back with concrete fixes.
5. Send the light 1x PNGs (and one dark example) to the maintainer as files, with the scenario index.
6. Ask for approval in German. Only after an explicit "approved"/"freigegeben": write
   `Design approved: <YYYY-MM-DD>` into `openspec/changes/$ARGUMENTS/design.md` and tick tasks 1.1/1.2.
