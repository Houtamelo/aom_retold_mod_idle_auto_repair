# test_targeting

Test mod for probing the engine-side auto-target acquisition logic in
AoM:R. See the parent conversation thread for the full reasoning behind
each test.

This mod overrides three units via `proto_mods.xml` and two tactics
files. After deploying, observe the behavior of Cyclops, Scarab, and
Quinametzin in-game.

## What this mod tests

### Test 1: Cyclops with MythUnitSiege KEPT + AttackPriorityType Villager +50

`MythUnitSiege` is preserved. A custom `cyclops_test.tactics` adds:

```xml
<AttackPriorityType bonusFactor="50">LogicalTypeVillagersAttack</AttackPriorityType>
```

- If `AttackPriorityType` is honored by the engine, Cyclops should now
  prefer villagers over buildings — i.e. NOT hard-focus buildings.
- If the field is ignored (or unused), Cyclops keeps its vanilla
  hard-focus behavior (MythUnitSiege drives it).

### Test 2: Scarab with MythUnitSiege REMOVED + AttackPriorityType Building +50

`MythUnitSiege` is removed via `<unittype mergeMode="remove">`. A custom
`scarab_test.tactics` adds:

```xml
<AttackPriorityType bonusFactor="50">Building</AttackPriorityType>
```

- If `AttackPriorityType` is the actual mechanism that drives building
  focus, Scarab should still focus buildings.
- If `MythUnitSiege` is the sole mechanism (and `AttackPriorityType`
  is dead code), Scarab will lose building focus entirely and behave
  like Minotaur.

### Test 3: Quinametzin with MythUnitRanged REMOVED

Just removes `MythUnitRanged`. Vanilla tactics unchanged.

- If `MythUnitRanged` is the sole trigger, Quinametzin loses any
  building-focus behavior.
- If there's a hidden mechanism, Quinametzin keeps focusing buildings
  even without the tag.

## Deploy

```bash
cd mod/test_targeting
python3 build.py                    # generates the .XMB files
./scripts/deploy-mods.sh test_targeting   # copies into the game's mods/local/
```

In-game, enable `test_targeting` from the mods menu and observe the
targeting behavior of Cyclops, Scarab, and Quinametzin.

## How to interpret results

| Test | Observed behavior                              | Conclusion                                              |
| ---- | ---------------------------------------------- | ------------------------------------------------------- |
| 1    | Cyclops still hard-focuses buildings            | `AttackPriorityType` is ignored; `MythUnitSiege` is the trigger. |
| 1    | Cyclops prefers nearby villagers                 | `AttackPriorityType` works; middle-ground targeting is possible. |
| 2    | Scarab still focuses buildings                    | `AttackPriorityType` can drive focus without the siege tag.       |
| 2    | Scarab attacks closest target only                | `MythUnitSiege` is the trigger; `AttackPriorityType` is decorative.  |
| 3    | Quinametzin still focuses buildings               | There's a hidden mechanism beyond `MythUnitRanged`.               |
| 3    | Quinametzin attacks closest target                | `MythUnitRanged` is the trigger.                                     |

## Files

```
mod/test_targeting/
├── README.md                   ← this file
├── build.py                    ← regenerates deployed artifacts
├── source/
│   ├── proto_mods.xml          ← merge directives (verbatim deployed)
│   └── tactics/
│       ├── cyclops_test.tactics.xml
│       └── scarab_test.tactics.xml
└── game/                       ← deployed artifacts (gitignored)
    └── data/
        └── gameplay/
            ├── proto_mods.xml
            └── tactics/
                ├── cyclops_test.tactics.XMB
                └── scarab_test.tactics.XMB
```

`game/` is regenerated from `source/` by `build.py`. The source XMLs
are the human-editable form; the `.XMB` files in `game/` are the
binary form the engine actually reads.