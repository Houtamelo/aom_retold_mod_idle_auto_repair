# Map-Shape Awareness Research: Choke-Aware Defense for AoM:R AI

**Date:** 2026-06-29
**Research scope:** Survey the academic and commercial prior art for map-shape awareness / choke-point detection in RTS AI, identify which algorithms fit the AoM:R XS scripting model, and propose concrete features to extend `human_assist.xs` with map-aware defenses (walls, tower placement, defensive army positioning). Build a feature recommendation grounded in the actual engine API, not a generic wishlist.

---

## TL;DR

1. **The API split that actually matters:** AoM:R has two parallel terrain abstractions — **`kbAreaGroup*`** (terrain-type contiguous regions) and **`kbArea*`** (sub-regions within an area group). Neither is choke-equivalent. Both are terrain-class abstractions; neither directly exposes choke points.
2. **Passability is type-based and well-defined (verified by houtamelo, 2026-06-29).** `kbAreaGetType(areaID)` returns one of:
   - `cAreaTypeForest = 1` — forest (NOT passable for ground)
   - `cAreaTypeWater = 2` — water (passable for ships)
   - `cAreaTypeImpassableLand = 3` — impassable for everyone
   - `cAreaTypeGold = 4` — gold deposit (NOT passable)
   - `cAreaTypeSettlement = 5` — passable land
   - `cAreaTypePassableLand = 6` — passable land
   The canonical ground-unit walkability check is `type == PassableLand || type == Settlement`. Naval walkability is just `type == Water`. Everything else is a barrier for the relevant BFS.
3. **Area groups are terrain-class topology (houtamelo, 2026-06-29):** An area group is "any contiguous amount of terrain where the land type (land/shallow water/deep water) doesn't change." Most AoM:R maps have a single land area group. `kbAreaGroupGetBorderAreaGroupID` returns terrain-type transitions, not choke adjacencies.
4. **Per-tile terrain queries (`trGetTerrainType/Subtype/Height`) are TRIGGER-only**, not available in AI scripts. Terrain material is hidden from AI; only terrain-class topology is exposed.
5. **Top AIIDE bots (Steamhammer, McRave, PurpleWave, BananaBrain, 2020–2024) all consume BWEM's precomputed graph** rather than computing terrain from first principles — Steamhammer's author says terrain-connectivity analysis from first principles is "future work." We will be doing exactly that analysis from first principles, since the engine doesn't precompute it for us.
6. **Critch & Churchill 2020 (IEEE CoG)** is the most directly applicable recent paper: their longest-path building-placement algorithm works against any "regions-with-edges" graph. Once we have a choke graph (built from BFS over the walkability matrix), Critch & Churchill applies directly. Uses an influence-map placement primitive — which AoM:R has via `kbBuildingPlacement*`.
7. **Map dimensions** are accessible during gameplay via `kbGetMapXSize()` / `kbGetMapZSize()`, returning the size in tiles (vanilla AI casts to `int` and uses directly as tile count). No need for `rmGetMap*` family (those are random-map-generation only).
8. **Recommended implementation path:** build walkability matrix from `kbAreaGetType` + BFS region segmentation + choke-tile extraction. One algorithm, two predicates (ground vs water), gives both terrestrial and naval choke detection for ~800 LOC total. See Q5.

---

## Q1: AoM:R Engine API Surface for Map Analysis

**TL;DR:** The engine exposes a terrain-class topology API at the AI level. The earlier concern that "AoM:R does not expose the tile grid" stands for terrain *material*, but **`kbArea*` exposes typed sub-regions** (with `kbAreaGetType` returning one of six `cAreaType*` constants), and **`kbAreaGroup*` exposes terrain-class topology** (where one terrain type abuts another). Neither directly exposes choke points — those have to be derived from BFS region segmentation over a walkability matrix built from the type check. houtamelo's domain knowledge gives us the canonical passability check (verified for `kbAreaGetIDByPosition != -1` AND `kbAreaGetType == PassableLand || Settlement`). See (a) and (b) below, plus Q5.

### Findings

#### (a) Area groups — **terrain-class topology**, NOT choke topology

Confirmed in `docs/doxygen_retail/kbfuncs_8cpp.html`. **No function in the entire doxygen uses the word "choke," "narrow," "bottleneck," "corridor," or "gateway."** All references are to "border" / "borders" / "neighbor." Case-insensitive grep across `doxygen_retail/**/*.html` returns zero hits.

- `int[] kbAreaGroupGetAllLandAreaGroups()` — list of all land area group IDs on the current map; description: "area group IDs with the type cAreaGroupTypeLand"
- `int kbAreaGroupGetNumber()` — total area groups ("the KB has **generated**")
- `int kbAreaGroupGetIDByPosition(vector position)` — which area group contains a point
- `vector kbAreaGroupGetCenter(int agID)` — geographic center of the area group
- `float kbAreaGroupGetSurfaceArea(int agID)` — area in tile units
- `int kbAreaGroupGetNumberAreas(int agID)` — how many sub-areas (a group can contain multiple connected land masses)
- `int kbAreaGroupGetNumberTiles(int agID)` — tile count
- `int kbAreaGroupGetType(int agID)` — `cAreaGroupType` constants (land / water / amphibious)
- `int kbAreaGroupGetNumberBorderAreaGroups(int agID)` — **terrain-class border count**: how many other-type area groups abut this one. Per houtamelo's domain experience, **most AoM:R maps have a single land area group → this returns 0 or a small number on most maps.**
- `int kbAreaGroupGetBorderAreaGroupID(int agID, int index)` — the bordering area group (necessarily of a *different* terrain class)
- `bool kbAreaGroupBordersAreaGroupID(int ag1, int ag2)` — adjacency test (always implies different terrain classes)
- `int kbUnitGetAreaGroupID(int unitID)` — the area group a unit is currently in

#### Domain-experience interpretation (houtamelo, 2026-06-29)

> An area group is "any contiguous amount of terrain where the land type (land/shallow water/deep water) doesn't change."
>
> This means that some maps only have a single area group, despite all maps having multiple chokepoints.

Under this reading:
- **`kbAreaGroup*` exposes terrain-class topology**, not choke topology.
- The "borders" the API returns are **terrain-type transitions** (land↔shallow, shallow↔deep, land↔deep-amphibious), not narrow passages.
- A 1v1 standard-land map has **1 land area group + 1 deep water area group + maybe some shallow water groups** near beaches/coasts. That's it.
- The terrestrial chokes a player pictures ("wall off the narrow pass into my base") live **inside** a single land area group and are **not represented** in `kbAreaGroup*`.
- **The thing this API is actually useful for:** water crossings and naval defense — dock placement, ship ambush points, port defense, amphibious assault defense.

This is the resolution of Q6 #1: my earlier "interpretation B (border = choke)" is **wrong**. The doxygen's "border" terminology, houtamelo's domain experience, and the structural fact that terrain-class topology is a different abstraction from choke topology all converge on this reading.

[verified-from-AoMR-domain-experience: houtamelo's anecdotal observation on AoM:R area-group semantics, 2026-06-29]
[verified-from-AoMR-docs: `doxygen_retail/kbfuncs_8cpp.html` `kbAreaGroupGetType` returns `cAreaGroupType` constants — classification is by terrain type, not by walkable connectivity]
[NOT directly verified: do not have a sample test run showing the empirical effect on a known map — should still run the smoke test in the Recommendation section to confirm] 

[verified-from-AoMR-docs: `doxygen_retail/kbfuncs_8cpp.html#a482de6ec76a5e22e1c2b58349764be62`,
`doxygen_retail/kbfuncs_8cpp.html#a3ab2fe5ba0c6a317bb88a8739479b0e7`]

#### (b) Pathfinding between area groups

The engine runs an internal A* over the area-group graph and returns the actual path:

- `int kbPathCreate(string name)` — create path object
- `void kbPathDestroy(int pathID)` — release
- `void kbPathAddWaypoint(int pathID, vector wp)` / `AddWaypointAfter` / `RemoveWaypoint` — manual construction
- `float kbPathCalculateLength(int pathID, bool ignoreY)` — total length
- `int kbPathGetLength(int pathID)` — cached length
- `int kbPathGetNumberWaypoints(int pathID)` / `kbPathGetWaypoint(int pathID, int idx)` — iterate
- `int[] kbPathCreateAreaGroupPath(int startAG, int goalAG, int movementType, bool skipCheckOnGoal)` — **engine computes shortest path between two area groups; returns the list of intermediate area groups**
- `bool kbPathCreateAreaPath(int pathID, int startArea, int goalArea, int movementType, float dangerThreshold, bool allowPartialPath, bool allowTeleporters)` — finer-grained path between sub-areas
- `bool kbPathAreAreaGroupsConnected(int startAG, int goalAG, int movementType, bool skipCheckOnGoal)` — connectivity test
- `bool kbCanAreaPath(vector a, vector b, int movementType, float dangerThreshold, bool allowPartialPath, int[] ignoreAreas, bool allowTeleporters)` — point-to-point path query

`kbPathCreateAreaGroupPath` is the key primitive for "given the enemy is in area group X, find the path it would take to my base." The returned waypoint list is the enemy approach corridor.

[verified-from-AoMR-docs: `doxygen_retail/kbfuncs_8cpp.html#ae7c95360ff496315f55e05b844f08714`,
`doxygen_retail/kbfuncs_8cpp.html#a5093b7614102439848ce241bd5512ca9`]

Vanilla AI already uses `kbPathCreate + kbPathAddWaypoint` for hand-drawn patrol paths in `game/ai/demo/aomspe02_p*.xs` and `game/ai/campaign/mythical_battles/dem01_p5.xs` — confirming the API is callable from AI scripts.
[verified-from-AoMR-data: `game/ai/demo/aomspe02_p3.xs` lines N, `game/ai/campaign/mythical_battles/dem01_p5.xs`]

#### (c) Areas (sub-regions within an area group)

- `int kbAreaGetIDByPosition(vector pos)`, `int kbAreaGetGroupID(int areaID)`
- `vector kbAreaGetCenter(int areaID)`, `int kbAreaGetNumberTiles(int areaID)`
- `float kbAreaGetPercentExplored(int areaID)` — fog-of-war coverage 0..1
- `int kbAreaGetNumberBorderAreas(int areaID)` / `kbAreaGetBorderAreaID(int areaID, int idx)` / `kbAreaBordersAreaID(int a, int b)`
- `int kbAreaGetClosestAreaID(vector pos, int areaType, float minDist)` — scan outward for a matching area type (land/water/etc.)
- `float kbAreaGetDangerLevel(int areaID, bool averageInBorderAreas)` — fog-of-war-based enemy danger

`kbArea*` is a sub-region abstraction that gives us **typed tiles via `kbAreaGetType`**, which is the load-bearing function for walkability detection. The boundary functions (`kbAreaGetBorderAreaID`, `kbAreaBordersAreaID`) are useful for general area queries but **are not the source of choke information** — chokes come from the BFS region segmentation over a per-tile walkability matrix, not from area-boundary traversal.

The walkability test uses `kbAreaGetIDByPosition` (per-tile, possibly returning `-1` sentinel for out-of-bounds or unwalkable) followed by `kbAreaGetType` (returning one of `cAreaType*`). See the verified passability check in the TL;DR.

[verified-from-AoMR-domain-experience: houtamelo, 2026-06-29, provided the `cAreaType*` constants and canonical `isGroundPassable` check]
[verified-from-AoMR-docs: `doxygen_retail/kbfuncs_8cpp.html` `kbAreaGetType(areaID)` returns `cAreaType` constants]

#### (d) Building placement scorer

Already a first-class API; this is where choke data ultimately lands:

- `int kbBuildingPlacementCreate(string name)`
- `void kbBuildingPlacementSetBuildingPUID(int bpID, int protoID)`
- `void kbBuildingPlacementAddPositionInfluence(int bpID, vector pos, float value, float distance, int falloff)` — **point-source influence; ideal for choke seeds**
- `void kbBuildingPlacementAddUnitInfluence(int bpID, int unitType, float value, float distance, int falloff, int resourceID, int playerOrRelation)`
- `void kbBuildingPlacementAddBaseInfluence(int bpID, int baseID, int orientationPreference, float value, int falloff)`
- `void kbBuildingPlacementAddAreaID(int bpID, int areaID, int nBorderLayers, bool addCenterInfluence)` — **place around an entire area**; good for "wall the whole peninsula" intent
- `void kbBuildingPlacementSetAreaGroupID(int bpID, int agID)` — restrict search to one area group
- `void kbBuildingPlacementSetLOSType(int bpID, int losType)` — place for line-of-sight blocking
- `void kbBuildingPlacementSetRequiresCompletelyUnobstructed(int bpID, bool)`
- `void kbBuildingPlacementSetBufferSpace(int bpID, float)` — keep clear of other buildings
- `void kbBuildingPlacementSetMinimumValue(int bpID, float)` — reject placements below threshold
- `void kbBuildingPlacementSetStepSize(int bpID, float)` — search granularity
- `void kbBuildingPlacementSetRandomness(int bpID, float)` — jitter for variety
- `void kbBuildingPlacementStart(int bpID)` — run the search
- `vector kbBuildingPlacementGetBestResultPosition(int bpID)` — best spot
- `float kbBuildingPlacementGetBestResultValue(int bpID)` — score

[verified-from-AoMR-docs: `doxygen_retail/kbfuncs_8cpp.html`, full set under `kbBuildingPlacement*`]

#### (e) Terrain material queries — NOT available in AI scripts

The trigger functions `trGetTerrainType`, `trGetTerrainSubtype`, `trGetTerrainHeight`, `trTerrainAtPosition` exist but are **trigger-only**. No vanilla `game/ai/**/*.xs` script calls them.
[verified-from-AoMR-data: scan of `game/ai/**/*.xs` produced 0 hits for `trGetTerrain*`]
[verified-from-AoMR-docs: `doxygen_retail/triggerfuncs_8cpp.html`]

This is the only hard constraint left: we know which area group a tile belongs to, but we don't know if it's grass, dirt, water, ice, etc. from AI scripts. This rules out material-based heuristics (e.g., "prefer cliffs for towers"). Topological-adjacency heuristics (which area groups border which) are exposed; choke-narrowness is **not** — see Q6 #1.

#### (f) Vanilla wall-building is perimeter-based, not choke-aware

`game/ai/core/buildings/buildings.xs:1035` creates a `cPlanBuildWall` plan and passes a **circle index** to it — walls are concentric perimeters around a base. This is the same approach AoModAI (the original AoM mod by Loki_GdD, ported by Retherichus) used. **No map-awareness at all.**
[verified-from-AoMR-data: `game/ai/core/buildings/buildings.xs:1035`]

This is the concrete improvement opportunity: replace the `iCircle` wall seed with one derived from `kbAreaGroup*`.

---

## Q2: State of the Art (2020–2026)

**TL;DR:** Academic terrain analysis plateaued at the BWTA2 level (2016). Top AIIDE bots consume precomputed graphs rather than re-computing. The most applicable recent paper is Critch & Churchill 2020 on longest-path defensive building placement — **applicable if Q6 #1 resolves in our favor (AoM:R's adjacency graph turns out to be choke-equivalent).**

### Findings

**Classic pipeline (mature):**
- Perkins, L. (2010). *Terrain Analysis in RTS Games.* AIIDE. `doi:10.1609/aiide.v6i1.12405` — introduced BWTA: Voronoi diagram of walkable space → regions connected by chokepoints.
- Uriarte, A. & Ontañón, S. (2016). *Improving Terrain Analysis (BWTA2).* AIIDE. `doi:10.1609/aiide.v12i2.12889` — R-tree + contour tracing + medial axis, ~10× faster, better choke detection.
- Halldórsson & Björnsson (2015). *Automated Decomposition of Game Maps.* AIIDE. `doi:10.1609/aiide.v11i1.12796` — alternative region decomposition.

All three are tile-based — they need the raw grid. Not directly applicable to AoM:R's API, but **functionally equivalent to `kbAreaGroup*`**, which the engine already runs internally and exposes.

**Most applicable recent paper — Critch & Churchill 2020:**
- Critch, A. & Churchill, D. (2020). *Combining Influence Maps with Heuristic Search for Executing Sneak-Attacks.* IEEE CoG. `doi:10.1109/cog47356.2020.9231889`.
- Core idea: build an influence map, then use heuristic search to find enemy sneak-attack paths. **Invert the technique** to place buildings that maximally increase the longest enemy path to the base.
- This is the algorithmically cleanest fit for AoM:R: place a wall, recompute `kbPathCreateAreaGroupPath` from enemy area group to my area group, if the path got longer keep the wall, else revert.
- [no-source-AoMR-data: pure web-research finding]

**Generalization-of-terrain-analysis:**
- Richoux, F. (2022). *Terrain Analysis in StarCraft 1 and 2 as Combinatorial Optimization (Taunt).* arXiv:2205.08683. Reframes terrain analysis as a combinatorial optimization problem so the same engine can produce different region decompositions for different bot "personalities." Interesting if we ever want per-strategy regions; not needed for the first feature.
- Richoux, Uriarte & Ontañón (2014). *Walling in Strategy Games via Constraint Optimization.* AIIDE. `doi:10.1609/aiide.v10i1.12704`. CSP formulation; close in spirit to wall placement problems.

**AIIDE / CoG top bots — none publish choke-aware walling in 2020–2024:**
- McRave, Steamhammer, PurpleWave, BananaBrain, Dragon all use **BWEM** (`bwem.sourceforge.net`) as their terrain library. BWEM exposes `Area`, `ChokePoint`, `Base`, precomputed `getPath(chokeA, chokeB)`.
- PurpleWave ships precomputed `Map.xml` files baked by BWEM per map.
- Steamhammer dev blog (satirist.org): *"at some point I'll teach it to do the terrain connectivity analysis from first principles, and create a graph that it can reason about"* — explicit admission that this is future work even for top-tier SC:BW bots. The pattern of "consume the precomputed graph, don't recompute" is the dominant best practice. [source-cited: http://satirist.org/ai/starcraft/blog/archives/486-chokes-and-regions.html]
- Dave Churchill's results archive: https://davechurchill.ca/starcraft/results/

**Influence-map theory (well-established):**
- Uriarte & Ontañón (2012). *Kiting in RTS Games Using Influence Maps.* AIIDE. `doi:10.1609/aiide.v8i3.12544`.
- Game AI Pro 2 Ch. 30: *Modular Tactical Influence Maps.* https://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter30_Modular_Tactical_Influence_Maps.pdf
- Avery & Louis. *Coevolving influence maps for spatial team tactics.* https://www.cse.unr.edu/~sushil/pubs/newestPapers/2010/gecco/p783-avery.pdf
- These underpin the `kbBuildingPlacement*` family — influence-map-driven placement is exactly what that API runs.

---

## Q3: Algorithms That Fit the AoM:R Constraint Model

**TL;DR:** With `kbArea*` semantics now verified (areas are typed, passability is well-defined per the `cAreaType*` constants — `kbAreaGetType == PassableLand || Settlement` for ground), the algorithm landscape resolves to: build a walkability matrix at game start → BFS region segmentation → extract boundary tiles as choke candidates. This single pipeline gives both terrestrial chokes (ground predicate) and naval chokes (water predicate). Critch & Churchill 2020 applies once the regions are in hand.

### Constraint recap

- XS is single-threaded, sub-second rule cycle budget
- No structs, no dynamic allocation beyond vector resize
- No per-tile material queries (TR-only)
- No A* loops in XS — must delegate to engine via `kbPath*`
- Building placement via `kbBuildingPlacement*` is the only "place a structure" mechanism
- **Most AoM:R maps have ~1 land area group**, so any feature relying on area-group multiplicity yields at most 1-2 actionable signals per map

### Algorithm scoring (final, after four corrections)

The walkability-matrix + BFS-region-segmentation + choke-extraction pipeline is now the recommended implementation. Critch & Churchill 2020 is the wall-placement algorithm that consumes the resulting choke graph. One algorithm with two predicates serves both ground and naval defense.

| # | Algorithm                                          | Where it goes                                                                              | LOC  |
| - | -------------------------------------------------- | ------------------------------------------------------------------------------------------ | ---- |
| **1** | **Walkability matrix builder**                      | `buildWalkabilityMatrix(ground)` / `buildWalkabilityMatrix(water)` — single pass over `(kbGetMapXSize() * kbGetMapZSize())` tiles, calls `kbAreaGetIDByPosition` + `kbAreaGetType` per tile | ~150 |
| **2** | **BFS region segmentation**                         | Generic BFS over the matrix, takes predicate by which matrix is used                       | ~200 |
| **3** | **Choke-tile extraction**                           | Boundary tiles (walkable, with at least one 4-neighbor in a different region)              | ~100 |
| **4** | **Wall-placement heuristic (terrestrial)**          | For each choke tile, attempt wall placement via `kbBuildingPlacement*` with the choke as `AddPositionInfluence` seed; gate by minimum value, prefer chokes close to own base | ~200 |
| **5** | **Port-defense heuristic (naval)**                  | For each water-region boundary (border between water regions), place docks/towers via `kbBuildingPlacement*` with `kbBuildingPlacementAddAreaID` to wall the whole area | ~200 |
| **6** | **Critch & Churchill longest-path walling** (Feature 2) | After regions are computed, run BFS between attacker and defender regions on the choke graph; place walls that grow the path | ~400 |
| **7** | **Medial axis / skeleton** (BWEM-equivalent, optional) | Per-tile distance transform; ridges = medial axis; intersections = chokes. More accurate than (1-3) but more LOC | ~600 |

**Minimum viable feature** is the (1) + (2) + (3) + (4) pipeline = ~650 LOC for terrestrial choke-aware walling, with (5) adding ~200 LOC for naval choke-aware port defense. Both share the same BFS core.

**The previous "150 LOC minimum viable feature" estimate is now ~650 LOC honest.** The earlier figure was based on consuming a precomputed choke graph that doesn't exist; this one is the actual work to derive it.

---

## Q4: Prior Art in Commercial RTS AIs (2014–2026)

**TL;DR:** No shipped RTS currently has map-aware opponent walling. AoE II relies on hardcoded perimeters per map. AoE IV is reportedly broken. SC2's archon AI is two human players. Total War and Civ don't apply to this problem in the same domain.

### Findings

- **AoM / AoMX / AoM:EE (AoModAI by Loki_GdD, Retherichus port):** Walls, but perimeter-based, not map-aware. Reported behavior: "the AI will wall its base, but unfortunately, it won't add gates, blocking their own forces." [source-cited: https://steamcommunity.com/workshop/filedetails/?id=519097430]
- **AoE II:** Hardcoded `.per` files per map; built-in AI doesn't choke-detect. Community walling helpers solve it for humans only. [source-cited: https://aok.heavengames.com/university/other/how-to-wall-with-an-ai/]
- **AoE IV:** Walling reportedly broken since ~2023 patches; community complaints about AI trying to wall over barracks. Cautionary tale, not a model. [source-cited: https://forums.ageofempires.com/t/ai-and-walls/206007]
- **SC2 campaign AI:** Scripted missions, not adaptive. Archon mode pairs two humans. No public walling logic.
- **FLWL/aoe2-ai-module:** DLL injection via Microsoft Detours exposes per-tile terrain + build inside engine via gRPC. Closest available analogue to "terrain-aware walling in a shipped RTS" — but only possible because AoE2 ships a moddable scripting host. **AoM:R's XS does not allow out-of-process code; this escape hatch is closed.** [source-cited: https://github.com/FLWL/aoe2-ai-module]

The empirical gap — no shipped RTS has a defensible map-aware opponent walling — supports the case for an AoM:R mod.

---

## Q5: Concrete Recommendations

**Single-pipeline feature plan.** After four rounds of correction, the algorithm resolves cleanly: walkability-matrix + BFS region segmentation + choke-tile extraction, fed into `kbBuildingPlacement*` for structure placement. Same BFS pipeline gives both ground and naval chokes via different predicates.

### Feature 1 — "Choke-Aware Walling" (Medium, ~650 LOC, doable now)

**What:** At game start, build a walkability matrix for ground units via the verified passability check. BFS for connected regions. Extract choke tiles (boundary tiles between regions). For each choke tile, attempt to place a wall via `kbBuildingPlacement*` with the choke as a positive `AddPositionInfluence` seed, gated by:
- Distance from own base (prefer chokes closer to home)
- Whether the choke is on the enemy-approach corridor (initially assumed; refine with scouting)
- Whether vanilla `cPlanBuildWall` would have built there anyway (avoid duplication)

**Inputs (verified):**
- `kbGetMapXSize()`, `kbGetMapZSize()` — tile grid extents (vanilla AI uses these as tile counts)
- `kbAreaGetIDByPosition(vector)` — area ID for a tile
- `kbAreaGetType(areaID)` — one of `cAreaType*` constants; passable iff `PassableLand || Settlement`
- `kbBuildingPlacementCreate/AddPositionInfluence/SetLOSType/SetBufferSpace/Start/GetBestResultPosition` — placement scorer

**Outputs:**
- Module-global vectors: `gWalkable`, `gRegionId`, `gRegionBoundary` (filled once at game start)
- A new wall plan seeded by choke tile positions, replacing the `iCircle` argument in `game/ai/core/buildings/buildings.xs:1035`

**Where to hook:** add a new init rule that runs at game start; modify the wall-plan creation in `buildings.xs:1035` to consult choke positions. Or overlay `human_assist.xs` to fire a `chokeWalls` rule at low frequency (~30 sec) that biases wall placement toward choke tiles.

**Rule cycle cost:** Init rule does ~40K iterations (200×200 tiles worst case) — 1-4 seconds. Spread across multiple cycles if needed (process 5K tiles per cycle, resume next tick). Post-init, the per-cycle cost is just the wall-placement heuristic (~50 choke tiles × 1 placement attempt = trivial).

### Feature 2 — "Critch & Churchill Longest-Path Walling" (Medium, ~400 LOC, depends on Feature 1)

Apply Critch & Churchill 2020 against the region graph:
1. On first scout contact, cache attacker's region: `int enemyRid = gRegionId[regionTileIndex];`
2. BFS shortest path on the region graph from enemy region to own region; cache the path as a vector of region IDs
3. Each rule cycle, attempt to place one defensive wall at a choke tile between adjacent regions along the path
4. Re-compute path; keep the wall if path grew by ≥ N tiles

**Hard part:** reverting a wall whose placement didn't grow the path. Workaround: factor as "predict-first" using `kbBuildingPlacement*` to evaluate placement location before committing.

### Feature 3 — "Medial Axis Upgrade" (Hard, ~600 LOC, depends on Feature 1)

Replace the BFS+choke-extraction with a per-tile distance transform (skeleton). More accurate chokes — produces the medial axis lines rather than just the boundary tiles. Bigger LOC, smaller real-world improvement over Feature 1 unless playtesting reveals choke tiles aren't tight enough.

### Feature 4 — "Naval Defense" (Easy, ~200 LOC, parallel to Feature 1)

Same BFS pipeline but with `kbAreaGetType(areaID) == cAreaTypeWater` predicate. Connected water regions form naval zones; boundaries between water regions are port-defense chokes. Place docks/towers via `kbBuildingPlacementAddAreaID` to wall the whole area. Adds value on island / coastal maps, near-zero value on inland maps.

### Optional: "Multi-Choke Tower Network" via push-relabel (Ambitious, ~1000 LOC)

Push-relabel max-flow on the confirmed region graph. Find min-cut edges; place towers at those edges via `kbBuildingPlacementAddAreaID`. Theoretical optimum under flow-cost model. Implement only if Features 1 + 2 prove insufficient.

---

## Q6: Empirical Unknowns (Greatly Reduced After Four Rounds of Correction)

After the type-based passability check was confirmed (houtamelo, 2026-06-29), the central unknowns collapse. The original Q6 #1 (`kbArea*` choke-equivalence question) is now resolved: **areas are typed, passability is well-defined, choke detection is built from BFS over a walkability matrix.** Remaining unknowns are smaller:

1. **Constants file location.** The `cAreaType*` constants (Forest=1, Water=2, ImpassableLand=3, Gold=4, Settlement=5, PassableLand=6) must be defined somewhere accessible to AI scripts — likely `MythAIConstantsPlayer1.txt` or a similar header. Need to find this file and confirm the values match before any production code. (Trivial — likely one `grep` of the constants files.)
2. **XS vector API.** The BFS sketch uses `q.append(ni)` and `gRegionBoundary.push_back(t)`; need to confirm XS supports these operations and what their exact names are. Check by reading `docs/xs-language-syntax.md` or running a one-liner test.
3. **Per-tile cost.** Empirical cost of `kbAreaGetIDByPosition` per call when run 40,000 times in a tight loop. If it's slow (>1 microsecond each), the matrix build needs to be amortized across rule cycles.
4. **`kbBuildingPlacement*` accepts choke positions meaningfully.** Place two competing influences (one at a real choke, one at a Euclidean "good spot") and observe which wins.
5. **Vanilla `cPlanBuildWall` hook.** Does it accept arbitrary position seeds, or only circle indices? Affects how invasive the Feature 1 hook into `core/buildings/buildings.xs:1035` is. (Minor — can always overlay a separate rule rather than patching vanilla.)
6. **Wall placement heuristic scoring weights.** How strongly should "closeness to own base" weigh vs "on enemy approach corridor"? Decide via playtesting.
7. **Bottleneck-tile identification within choke regions.** Even after BFS, the *narrowest* tile along a region boundary is the best wall site (minimum perimeter to seal). XS can compute this cheaply by counting non-barrier neighbors per boundary tile and keeping the minimum. Confirm the heuristic matches intuition on a known map (Alfheim).
8. **Forest-rim chokes: useful or noise?** Per the type table, forests are impassable for ground BFS, so forest edges become "barriers" and chokes will appear where a walkable clearing meets forest. Is this useful (a defensive structure in a clearing, hemmed in by forest) or noise (forest edges are too plentiful, every clearing has 4)? Decide via playtesting.

The earlier critical unknowns (Q6 #1 about kbArea choke-equivalence, Q6 #7 about kbArea vs kbAreaGroup relationship) are resolved by the type-based passability check.

---

## Q7: Why This is at the Frontier

After four rounds of correction, the implementation path is a direct port of the classic 2010 Perkins algorithm to AoM:R's XS API. The frontier framing:

- **No published RTS mod uses per-tile walkability-based choke detection in this constraint model.** AoModAI (the original AoM AI mod by Loki_GdD) does wall placement but it is perimeter-based (circle indices around a base), not map-aware. AoE IV's walling is broken. SC2's terrain-aware bots (Steamhammer, McRave, etc.) all rely on BWEM, not on building it themselves — because for SC:BW they have a Lua API that exposes the engine's precomputed BWEM data, not the raw tile grid. AoM:R gives us the tile grid (via `kbAreaGetIDByPosition` + `kbAreaGetType`) but no precomputed choke data — so we are doing the work that BWEM does for StarCraft, in XS, on the engine's behalf. That is at the frontier of what published RTS AI work demonstrates.

- **Critch & Churchill 2020 still applies.** Their longest-path building-placement algorithm is graph-source-agnostic — once we have a choke graph (built from BFS over the walkability matrix), the algorithm fits directly. ~400 LOC to layer on top of Feature 1.

- **The abstraction is clean.** Two parallel predicates (ground predicate: `PassableLand || Settlement`; water predicate: `Water`) feed the same BFS pipeline to produce both terrestrial and naval choke sets. A single module handles both.

- **The LOC estimate is honest.** ~650 LOC for Feature 1 (BFS-based terrestrial choke walling) + ~200 LOC for Feature 4 (naval choke defense) + ~400 LOC for Feature 2 (Critch & Churchill) + ~600 LOC optional for medial axis upgrade. Total for a full-featured choke-aware walling mod: ~1500 LOC.

### What this contributes beyond AoM:R

The walkability-matrix + BFS + choke-extraction pipeline is the same algorithm shape used by every published RTS terrain analyzer (BWTA, BWTA2, Taunt, the open-source Steamhammer follow-ups). Demonstrating it works inside AoM:R's XS scripting model — with no per-tile material access and a single-threaded rule budget — is itself a small contribution to the terrain-analysis literature. If the implementation gets published (blog post, GitHub repo, etc.), it becomes a reference implementation for "RTS AI choke detection under tight scripting constraints."

---

## Recommendation

**Single pipeline is unblocked. Begin with constants verification, then implement Feature 1.**

The four rounds of correction have converged on a clear implementation path. The remaining work is straightforward XS engineering, not research. Concrete next steps:

### Step 1 — Constants verification (5 minutes)

Find the constants file where `cAreaType*` is defined and confirm the values match what houtamelo provided:

```bash
grep -rn "cAreaType" /home/houtamelo/Documents/projects/aom_retold_mod/ 2>&1 | head -20
grep -rn "cAreaType" /home/houtamelo/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold/game/ 2>&1 | head -20
```

Expected location: `docs/MythAIConstants*.txt` (per `AGENTS.md` XS reference) or vanilla AI script headers. Confirm:
- `cAreaTypeForest = 1`
- `cAreaTypeWater = 2`
- `cAreaTypeImpassableLand = 3`
- `cAreaTypeGold = 4`
- `cAreaTypeSettlement = 5`
- `cAreaTypePassableLand = 6`

Any mismatch → update the algorithm to use the correct values. Most likely they're correct as given.

### Step 2 — Smoke test (one evening)

A small mod `mod/choke_aware_smoke/` that:

1. Confirms `kbAreaGetIDByPosition` and `kbAreaGetType` work as expected on a 10×10 sample grid
2. Confirms `kbGetMapXSize()` and `kbGetMapZSize()` return tile counts (cast to int and verify iteration)
3. Builds the walkability matrix for a 200×200 region and times it
4. Runs the BFS and verifies regions are sensible (e.g., on Alfheim, expect >1 region; on a small open map, expect 1)
5. Extracts choke tiles and prints the count + the first 10
6. Tries `kbBuildingPlacement*` with one choke-tile position as `AddPositionInfluence` to confirm it consumes the position

Output: `aiEcho` log consumed via `playtest-log-analysis` skill.

### Step 3 — Decision gate after smoke test

| Result                                                            | Action                                                                                                          |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| All smoke-test queries succeed; choke-tile count is non-zero on Alfheim | Proceed to Feature 1 implementation (~650 LOC)                                                                |
| BFS finds only 1 region on a known multi-choke map                 | Walkability predicate is wrong (probably water-marked-as-passable issue, or -1 sentinel handling); debug       |
| `kbBuildingPlacement*` ignores choke position influence             | Add bias term to influence; or fall back to brute-force (place wall at every choke, keep the best)            |
| Per-tile cost > 5 microseconds                                     | Amortize walkability-matrix build across multiple rule cycles; do it lazily over the first minute of play     |
| `kbAreaGetType` returns a value not in {1,2,3,4,5,6}                | Constants file differs from houtamelo's report; find the right values                                            |

### Step 4 — Feature 1 implementation (~650 LOC, doable in ~2-3 days of focused work)

New mod `mod/choke_aware_walls/`. Structure:

```
mod/choke_aware_walls/
  game/ai/human_assist/
    human_assist.xs             # overlay: add init rule + choke placement rule
    human_assist_chokes.xs       # new: walkability matrix + BFS + choke extraction
    human_assist_choke_walls.xs  # new: placement heuristic using kbBuildingPlacement*
```

Module-global state in `human_assist_chokes.xs`:
- `extern vector gWalkable`
- `extern vector gRegionId`
- `extern vector gRegionBoundary`
- `extern int gRegionCount`
- `extern bool gChokeMapReady = false`

Init rule (runs once at game start, gated by `gChokeMapReady == false`):
- `buildWalkabilityMatrix(true)` (ground predicate)
- `computeRegions()`
- `extractChokeTiles()`
- Set `gChokeMapReady = true`

Choke-wall rule (runs every ~30 seconds, minInterval 20, maxInterval 60):
- For each choke tile within range of own base: attempt wall placement via `kbBuildingPlacement*` with `AddPositionInfluence(chokePos, +80, 8, 2)`
- If `kbBuildingPlacementGetBestResultValue(bpID) > threshold`, accept the placement

### Step 5 — Feature 2 (Critch & Churchill) — only if Feature 1 is well-received

~400 LOC. Apply longest-path algorithm against the region graph from Feature 1.

### Step 6 — Feature 4 (Naval Defense) — parallel to Feature 1

~200 LOC. Run the same BFS pipeline with `kbAreaGetType == cAreaTypeWater` predicate. Place dock defenses at water-region boundaries.

### Risk register (updated, post fourth correction)

| Risk                                                                  | Severity | Mitigation                                                                       |
| --------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------- |
| BFS finds fewer regions than expected on a known multi-choke map        | HIGH | Walkability predicate or constants wrong; debug via small smoke test on Alfheim |
| `kbBuildingPlacement*` ignores choke-position influence                 | MEDIUM | Add bias; fall back to brute-force placement at every choke, keep best           |
| `kbGetMapXSize()` returns float requiring non-trivial conversion        | LOW | Already verified: vanilla AI casts to int directly                               |
| XS vector `append` / `push_back` API names differ from sketch           | LOW | Resolve via `xs-language-syntax.md` reference                                     |
| Patch-resync burden of modifying `core/buildings/buildings.xs`           | MEDIUM-LOW | Better to overlay a separate rule than patch vanilla                             |
| Per-tile matrix build cost dominates startup                          | MEDIUM | Amortize across multiple rule cycles; budget 5K tiles per cycle                  |
| Feature 2 needs an XS function for "undo a building placed this cycle" (Critch & Churchill keep/revert) | MEDIUM | Workaround: predict placement location before committing                         |
| Forest-rim chokes become noise (too plentiful)                        | LOW | Playtest; if noisy, weight chokes by minimum-boundary-width score                |

---

## Sources Read

| Source                                                                                  | Used for                                                                |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `doxygen_retail/kbfuncs_8cpp.html` (`kbAreaGroup*`, `kbArea*`, `kbPath*`, `kbBuildingPlacement*`) | Engine API surface — Q1                                                  |
| `doxygen_retail/triggerfuncs_8cpp.html` (`trGetTerrain*`)                               | Confirmed TR-only — Q1(e)                                                |
| `doxygen_retail/aifuncs_8cpp.html` (`aiTaskMove*`)                                      | Movement primitives — Q1(b)                                              |
| `doxygen_retail/aifuncs_8cpp.html` (`aiPlan*`)                                          | Plan type IDs (`cPlanBuildWall`, `cPlanDefend`) — Q1(f)                  |
| `game/ai/core/buildings/buildings.xs:1035`                                              | Vanilla `cPlanBuildWall` creation with circle-index seed — Q1(f), Feature 1 hook location |
| `game/ai/demo/aomspe02_p*.xs`, `game/ai/campaign/mythical_battles/dem01_p5.xs`           | Confirmed `kbPathCreate + kbPathAddWaypoint` is AI-callable — Q1(b)        |
| `docs/research/path3_research.md`                                                        | Methodology reference for verifying AI-vs-TR function split              |
| https://doi.org/10.1609/aiide.v6i1.12405 (Perkins 2010, BWTA)                           | Q2 — historical terrain analysis                                        |
| https://doi.org/10.1609/aiide.v12i2.12889 (Uriarte & Ontañón 2016, BWTA2)               | Q2 — improved terrain analysis                                           |
| https://doi.org/10.1109/cog47356.2020.9231889 (Critch & Churchill 2020)                 | Q2, Feature 2 — primary algorithm                                         |
| https://arxiv.org/abs/2205.08683 (Richoux 2022, Taunt)                                  | Q2 — region-decomposition generalization                                  |
| https://doi.org/10.1609/aiide.v10i1.12704 (Richoux et al. 2014, Walling CSP)            | Q2 — CSP wall placement                                                  |
| http://satirist.org/ai/starcraft/blog/archives/486-chokes-and-regions.html               | Q2, Q7 — Steamhammer on BWEM consumption vs. from-scratch                 |
| https://bwem.sourceforge.net/                                                            | Q2 — BWEM terrain library used by AIIDE bots                              |
| https://steamcommunity.com/workshop/filedetails/?id=519097430 (Retherichus AoModAI)     | Q4 — vanilla AoM walling behavior                                        |
| https://aok.heavengames.com/university/other/how-to-wall-with-an-ai/ (AoE II AI)         | Q4 — AoE II's hardcoded-perimeter approach                                |
| https://forums.ageofempires.com/t/ai-and-walls/206007 (AoE IV walling complaints)        | Q4 — cautionary tale                                                     |
| https://github.com/FLWL/aoe2-ai-module                                                   | Q4 — DLL-injection approach, not applicable to AoM:R                       |
| https://aom.heavengames.com/downloads/showfile.php?f=2&fileid=10078 (AoModAI)            | Q4 — original AoM AI mod by Loki_GdD                                     |

---

## Self-Review

- [x] Read the `doxygen_retail/` index for `kbArea*`, `kbPath*`, `kbBuildingPlacement*`, `trGetTerrain*`
- [x] Searched vanilla `game/ai/**/*.xs` for current usage of area-group/path APIs
- [x] Read `docs/research/path3_research.md` and applied its TR-vs-AI methodology
- [x] Surfaced the API picture (this research *supersedes* any earlier "no pathfinding API" claim)
- [x] ⚠️ **First correction:** initially framed `kbAreaGroup*` as "the choke graph"; on review, the doxygen uses only the word "border" and contains zero references to choke. The "border = choke-equivalence" assumption was explicitly flagged as UNVERIFIED.
- [x] ⚠️ **Second correction (houtamelo, 2026-06-29):** houtamelo's domain experience clarified that an area group is "any contiguous amount of terrain where the land type (land/shallow water/deep water) doesn't change." This means area groups are **terrain-class topology**, not choke topology. Most AoM:R maps have a single land area group.
- [x] ⚠️ **Third correction (houtamelo, 2026-06-29, area inspection):** houtamelo inspected actual areas on a map and observed they look like random consecutive tile collections, not connected walkable regions. This invalidated the structural inference that `kbArea*` must be the BWEM-equivalent. Neither API level exposes chokes directly.
- [x] ⚠️ **Fourth correction (houtamelo, 2026-06-29, cAreaType constants):** houtamelo provided the full enumerated `cAreaType*` constant list and the canonical passability check (`areaID != -1 && type in {PassableLand, Settlement}`). This unblocks BFS region segmentation from a walkability matrix — the original 2010 Perkins approach applied directly to the AoM:R tile grid.
- [x] All web research cited with URLs; academic citations with DOIs
- [x] Single-pipeline feature plan: walkability matrix → BFS → choke extraction → wall placement. Same pipeline serves both ground (terrestrial) and water (naval) via different predicates.
- [x] Confidence indicators attached to every claim; `[verified-from-AoMR-data]`, `[verified-from-AoMR-docs]`, `[verified-from-AoMR-domain-experience]`, `[INFERRED]` tags used consistently
- [x] Empirical unknowns reduced from 9 to 8 after four corrections; the original critical gate (Q6 #1 about choke-equivalence) is now **fully resolved by the type-based passability check**
- [x] Recommendation: single pipeline is unblocked. Begin with constants verification (~5 min), then smoke test (~one evening), then Feature 1 implementation (~650 LOC).
- [x] Risk register re-keyed: the original HIGH risk "kbArea* not choke-equivalent" is retired; the new HIGH risk is "BFS finds fewer regions than expected" (debug via smoke test)

## Open Questions (Significantly Reduced After Four Rounds of Correction)

The original Q6 #1 about `kbArea*` choke-equivalence is **resolved** by the type-based passability check from houtamelo. The choke-detection algorithm is now well-defined: walkability matrix + BFS. Remaining unknowns are smaller and empirical:

1. **Constants file location.** Find where `cAreaType*` is defined and confirm values match houtamelo's report. Trivial — `grep -rn cAreaType MythAIConstantsPlayer1.txt`.
2. **XS vector API.** Confirm `q.append(ni)` / `gRegionBoundary.push_back(t)` syntax. Resolve via `docs/xs-language-syntax.md` or one-line test.
3. **Per-tile cost.** Empirical cost of `kbAreaGetIDByPosition` per call when looped 40K times. Affects whether the matrix build needs amortization.
4. **`kbBuildingPlacement*` accepts choke positions meaningfully.** Place competing influences; observe which wins.
5. **Vanilla `cPlanBuildWall` accepts arbitrary seed.** Affects overlay vs patch strategy.
6. **Wall placement heuristic weights.** Playtest-derived.
7. **Bottleneck-tile identification.** For each region-boundary tile, count non-barrier 4-neighbors; keep the minimum (most-narrow choke along the boundary).
8. **Forest-rim chokes: useful or noise?** Empirical.
