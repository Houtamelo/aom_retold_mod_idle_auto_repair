# proto_mods.xml Syntax Reference

**Purpose:** Documents the correct syntax for AoMR's additive-merge `proto_mods.xml` file.
**Used by:** Task 3 (adding protoaction + flags to VillagerGreek).

---

## Q1: Root Element Name

**Root element is `<protomods>`.**
Confidence: **verified-from-AoMR-docs** (CryBarEditor Modding.md) and **verified-from-AoMR-mod** (MCMT `proto_mods.xml`).

The file lives at: `game/data/gameplay/proto_mods.xml`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<protomods>
    <!-- unit entries here -->
</protomods>
```

Additive file naming follows the pattern: base filename + `_mods`, root element = base name + `mods`.
| Base file | Mod file | Root element |
|---|---|---|
| `proto.xml` | `proto_mods.xml` | `<protomods>` |
| `techtree.xml` | `techtree_mods.xml` | `<techtreemods>` |
| `powers.xml` | `powers_mods.xml` | `<powersmods>` |

---

## Q2: Per-Unit Additive Merge Syntax

**To modify an existing unit: use `<unit name="X">` with NO `mergeMode` attribute on the unit element.**
Confidence: **verified-from-AoMR-docs** and **verified-from-AoMR-mod**.

The default `mergeMode` is `modify`: replaces a child element if one with the same identity already exists, otherwise adds it. This is what you want for merging into existing units.

```xml
<protomods>
    <!-- Modify an existing unit (default mergeMode="modify" on unit) -->
    <unit name="VillagerGreek">
        <lifespan>30.0000</lifespan>
        <unittype>AbstractScout</unittype>
    </unit>

    <!-- Add a brand-new unit (also no mergeMode on unit element; game detects it's new) -->
    <unit name="CoolerHoplite">
        <displaynameid>STR_UNIT_COOLER_HOPLITE_NAME</displaynameid>
        <icon>resources\greek\player_color\units\cooler_hoplite_icon.png</icon>
    </unit>

    <!-- Remove an existing unit entirely -->
    <unit mergeMode="remove" name="Hoplite" />
</protomods>
```

The `mergeMode` attribute on the `<unit>` element itself is only used when **removing** a unit (`mergeMode="remove"`). For both adding new units and modifying existing ones, omit the `mergeMode` on `<unit>`.

Child elements inside a unit use `mergeMode` to control granular behavior:
- `mergeMode="modify"` (default) — replace matching child if it exists, otherwise add it
- `mergeMode="replace"` — replace an existing child element
- `mergeMode="remove"` — remove a specific child element
- `mergeMode="add"` — force-add as a new element (even if one with same identity exists)

---

## Q3: Adding a New `<protoaction>` to an Existing Unit

**Add a `<protoaction>` block inside the unit. Matching is by the `<name>` inner value.**
Confidence: **verified-from-AoMR-docs** (CryBarEditor) and **verified-from-AoMR-mod** (MCMT modifies BattleBoar, Raven, etc.).

- If the unit already has a protoaction with the same `<name>`, the engine merges/modifies it.
- If no protoaction with that `<name>` exists on the unit, it is added as new (default `modify` behavior = add if not found).
- For an unambiguously **new** protoaction (no existing match expected), this pattern is sufficient — the default `modify` on each child means "add if not present".

```xml
<protomods>
    <unit name="VillagerGreek">
        <protoaction>
            <name>AutoRepair</name>
            <type>AutoRepair</type>
            <persistent>1</persistent>
            <rof>1.500000</rof>
            <modifyamount>3.000000</modifyamount>
            <maxrange>5.000000</maxrange>
        </protoaction>
    </unit>
</protomods>
```

Real-world example from MCMT (adding RangedAttack to a unit that previously had none):
```xml
<unit name="TravelerOxWagon" mergeMode="add">
    <protoaction>
        <name>RangedAttack</name>
        <rof>1.000000</rof>
        <damage type="Pierce">6.000000</damage>
        <maxrange>18.000000</maxrange>
        <projectile>ProjectileArrow</projectile>
    </protoaction>
</unit>
```

Note: The MCMT example uses `mergeMode="add"` on the `<unit>` element for entirely new units being created. For VillagerGreek (an existing unit), omit the `mergeMode` on `<unit>`.

---

## Q4: Adding New `<flag>` Entries to an Existing Unit

**Flags match by their inner text value. A `<flag>` without `mergeMode` uses the default `modify` mode, which adds the flag if no flag with that text already exists.**
Confidence: **verified-from-AoMR-docs** (CryBarEditor node-matching rules for flags).

Flags do NOT have a name attribute — they are identified by their inner value (e.g., `NotSelectable`). Because each flag's identity is its text content, and the default `modify` adds a node if not found, adding a new flag simply appends it to the unit's flag list.

```xml
<protomods>
    <unit name="VillagerGreek">
        <!-- These flags are added to (not replace) the unit's existing flag list -->
        <flag>AutoRepairBuildings</flag>
        <flag>SomeOtherFlag</flag>
    </unit>
</protomods>
```

To remove a specific flag without touching others:
```xml
<flag mergeMode="remove">NotSelectable</flag>
```

**Flags append, not replace.** The `modify` default adds each flag as a new entry if that text value isn't already present.

---

## Combined Example for Task 3

This is the pattern Task 3 should use to add one `<protoaction>` and two `<flag>` entries to `VillagerGreek`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<protomods>
    <unit name="VillagerGreek">
        <flag>FlagOne</flag>
        <flag>FlagTwo</flag>
        <protoaction>
            <name>NewActionName</name>
            <type>AutoRepair</type>
            <!-- ... action-specific child elements ... -->
        </protoaction>
    </unit>
</protomods>
```

No `mergeMode` on `<unit>` (VillagerGreek already exists). No `mergeMode` on `<flag>` (default `modify` appends new flags). No `mergeMode` needed on `<protoaction>` unless overriding an existing one with the same name.

---

## Sources

1. **CryBarEditor Modding.md** (primary AoMR modding reference, open-source tool by CryShana)
   https://github.com/CryShana/CryBarEditor/blob/main/Documentation/Modding.md
   — Provides definitive table of root elements, mergeMode definitions, XML examples for proto_mods.xml, flag matching rules, protoaction matching rules.

2. **MCMT mod `proto_mods.xml`** (real working AoMR mod on GitHub by akinizer)
   https://github.com/akinizer/MCMT/blob/main/game/data/gameplay/proto_mods.xml
   — Raw 665 KB working file confirming `<protomods>` root, unit-level omit-mergeMode pattern for modifying existing units (BattleBoar, Raven), and protoaction/flag addition in practice.

3. **AoE Forums: Add or Replace Units thread**
   https://forums.ageofempires.com/t/add-or-replace-units/261152
   — Confirms `proto_mods.xml` filename requirement (not converted to `.xmb`); shows Cyclops protoaction modification example.

4. **Steam Community: Modding AoMR for Beginners (guide ID 3332716391)**
   https://steamcommunity.com/sharedfiles/filedetails/?id=3332716391
   — References CryBarEditor docs; confirms file placement at `game/data/gameplay/`.
