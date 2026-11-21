#!/usr/bin/env bash
# In-game check for Integrated Tunnels slice 1: an Item Importer part on a
# tunnel cable face pulls items out of the chest behind that face into the
# cable network's item store at the jar's rate, an Item Exporter part hands them
# on into the chest behind its own face, and the cable's
# `/integratedtunnels info` readout shows the parts, the store and the status.
#
# Every mod command is typed with `ctl chat "/cmd"`: a player's chat line is what
# the game parses and hands to `run_mod_command`, while `ctl command` runs the
# console path.
#
# The End, in the void at x=200 (away from the central island, its dragon egg
# and the ambient item entities of a generated level), a stone floor at y=100.
#
# This mod is ONE package now (the SDK layout): the carrier block, the two part
# items and the code travel in the same `.hmod`, so the game that runs this test
# is built by the launcher, which compiles `src/` against the pinned build:
#
#   horizon-launcher native-build <this port> --instance <i> --trust-native-code
#   GAME=<that instance's horizon-client> tools/e2e.sh
#
# `GAME` is required (there is no port-built game any more), and there is no
# `--dev-mod`: the package's code, assets and data all travel in the `.hmod` the
# launcher compiled in.
#
# What the run proves:
#  A  a chest of 6400 items behind an importer drains into the network store and
#     NOT in one tick: a sample taken a few ticks in is non-zero and far below
#     6400 (the jar's rate is 64 items per part per tick);
#  A2 the source empties and the store holds exactly what the chest held;
#  B  an exporter on another cable face hands the store to a second chest, which
#     fills to its own capacity; store + sink still holds every item (nothing is
#     lost when the destination refuses the rest);
#  C  with the exporter part removed the store grows and the sink stays put: the
#     network store is the medium, not a block-to-block hop (the control for B);
#  D  with the importer part removed a full source chest stays full: the importer
#     is the only source (the control for A);
#  E  both parts back: the source drains into the store again;
#  F  a part on a face with no container behind it does nothing;
#  G  right-clicking a cable face with an Item Importer places the part
#     (upstream's `ItemPart.useOn`);
#  I  a full water cauldron behind a Fluid Importer drains into the network's
#     fluid channel (1000 mB), leaving the cauldron empty;
#  J  a Fluid Exporter on the same network fills an empty cauldron from that
#     channel, and the channel is empty afterwards (1000 mB conserved: a full
#     cauldron's three whole levels);
#  K  with the Fluid Exporter removed the channel keeps its fluid and the sink
#     cauldron stays empty (the differential for J), and with the Fluid Importer
#     removed a refilled source stays full (the differential for I);
#  L  both fluid part item ids are real items;
#  M  the carrier cable's carried loot table rolls the cable.
#
# SELFTEST=1 flips phase B's conservation total (it must FAIL), proving the
# scenario can fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${IT_DEBUG:-}" ] || set -x

SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/integratedtunnels
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/integratedtunnels-e2e.sock}
WORLD=IntegratedTunnelsE2E
# Where the arena stands: the End's void, far from the central island.
X0=200
Y=101
# A chest's 27 slots of a 64-stack item; the sink's capacity in phase B.
SINK_CAPACITY=1728
# What the source chest is filled with (one oversized stack, so the 64-per-tick
# rate is visible for ~100 ticks).
SOURCE_FILL=6400
# Phase B: store + sink must hold every item (SELFTEST asks for one more).
EXPECT_CONSERVED=6400
if [ "$SELFTEST" = 1 ]; then EXPECT_CONSERVED=6401; fi
# Phase A's rate bound: 64 items per tick, with slack for the ~15 ticks the
# readout takes to arrive. An unlimited importer moves all 6400 on the first
# tick and fails this.
RATE_BOUND=2560

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -n "$GAME" ] || {
    echo "GAME must be a launcher-built horizon-client carrying this port:" >&2
    echo "  horizon-launcher native-build $PORT --instance <i> --trust-native-code" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/integratedtunnels-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    # No `--dev-mod`: the launcher-built game carries the package's code, assets and data.
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    echo "  launching $world ($GAME)" >&2
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    echo "  launched; waiting for the world" >&2
    ctl wait-ready 120
    ctl wait 40
    [ "$(ctl state | jq -r '.screen')" = playing ] || fail "the game is not playing"
    echo "  playing" >&2
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.block // empty'; }
cauldron_is() { # cauldron_is X Y Z EXPECTED  (the ctl `block` state text)
    local got
    got=$(ctl block "$1" "$2" "$3" | jq -r '.state // empty')
    [ "$got" = "$4" ] || fail "the block at $1,$2,$3 is '$got', expected '$4'"
    echo "  block($1,$2,$3) $4: ok" >&2
}
# The non-null slots of the container at a position, as JSON.
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[]? | select(. != null)]'; }
# The item count a container holds, over every slot.
total() { slots "$1" "$2" "$3" | jq '[.[] | .count] | add // 0'; }
has() { # has X Y Z ITEM COUNT
    local got
    got=$(slots "$1" "$2" "$3")
    jq -e --arg item "$4" --argjson count "$5" \
        '[.[] | select(.item == $item) | .count] | add // 0 | . == $count' <<<"$got" >/dev/null \
        || fail "expected $5 x $4 at $1,$2,$3; got $got"
    echo "  has($1,$2,$3) $4 x$5: ok" >&2
}

# The End's void at X0: a stone floor and clear air above it. The player is
# teleported first, so the chunks the fills touch are loaded.
arena() {
    echo "  arena: loading the End at x=$X0" >&2
    ctl command "execute in minecraft:the_end run tp @s $X0.5 $Y 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 8)) 100 -14 $((X0 + 12)) 100 14 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 8)) $Y -14 $((X0 + 12)) 108 14 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 $Y 0.5" >/dev/null
    ctl wait 20
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" $Y 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
    echo "  arena: ready" >&2
}

set_block() { # set_block X Y Z STATE
    ctl command "execute in minecraft:the_end run setblock $1 $2 $3 $4" >/dev/null
    ctl wait 5 >/dev/null
}

# The chat history holds the newest 100 lines, so a *count* of matching lines
# saturates on a long run (the fluid phases pushed past it) and can no longer tell
# a fresh reply from an old one. Each readout is therefore bracketed by a unique
# player-chat marker, and the lines older than that marker are the reply to it:
# the game's reply is printed first, then the marker line, and `messages` is
# newest first.
read_info() { # read_info X Y Z -- sets PARTS_LINE STORE_LINE FLUIDS_LINE STORE STATUS FLUID_TOTAL
    local marker="it-marker-$$-${RANDOM}-$1-$2-$3"
    local json seg
    say "integratedtunnels info $1 $2 $3"
    ctl chat "$marker" >/dev/null
    for _ in $(seq 1 40); do
        ctl wait 2 >/dev/null
        json=$(ctl messages 100)
        if jq -e --arg m "$marker" '[.chat[]? | select(contains($m))] | length > 0' <<<"$json" >/dev/null; then
            break
        fi
    done
    jq -e --arg m "$marker" '[.chat[]? | select(contains($m))] | length > 0' <<<"$json" >/dev/null \
        || fail "the marker line $marker never reached the chat"
    # The reply is printed before the marker, so it sits just *older* than it
    # (newest first): the block right after the marker, never the whole history.
    seg=$(jq -c --arg m "$marker" \
        '(.chat | to_entries | map(select(.value | contains($m))) | .[0].key) as $i | .chat[($i+1):($i+15)]' <<<"$json")
    PARTS_LINE=$(jq -r '[.[] | select(contains("parts "))] | .[0] // empty' <<<"$seg")
    STORE_LINE=$(jq -r '[.[] | select(contains("store "))] | .[0] // empty' <<<"$seg")
    FLUIDS_LINE=$(jq -r '[.[] | select(contains("fluids "))] | .[0] // empty' <<<"$seg")
    [ -n "$STORE_LINE" ] || fail "no /integratedtunnels info reply"
    [ -n "$PARTS_LINE" ] || fail "no /integratedtunnels parts reply"
    [ -n "$FLUIDS_LINE" ] || fail "no /integratedtunnels fluids reply"
    STORE=$(grep -o '[0-9]* items' <<<"$STORE_LINE" | head -1 | awk '{print $1}')
    STATUS=$(sed -n 's/.*status \([a-z]*\).*/\1/p' <<<"$STORE_LINE")
    FLUID_TOTAL=$(grep -o '[0-9]* mB' <<<"$FLUIDS_LINE" | head -1 | awk '{print $1}')
    [ -n "$STORE" ] || fail "the store line did not parse: $STORE_LINE"
    [ -n "$FLUID_TOTAL" ] || fail "the fluid line did not parse: $FLUIDS_LINE"
    echo "  $PARTS_LINE" >&2
    echo "  $STORE_LINE" >&2
    echo "  $FLUIDS_LINE" >&2
}

# `/integratedtunnels info X Y Z`: the parts, the item store, the fluid channel.
info() { # info X Y Z
    read_info "$@"
}

expect_store() { # expect_store VALUE LABEL
    [ "$STORE" = "$1" ] || fail "$2: the network store holds $STORE items, expected $1"
}

expect_fluid() { # expect_fluid VALUE LABEL
    [ "$FLUID_TOTAL" = "$1" ] || fail "$2: the fluid channel holds $FLUID_TOTAL mB, expected $1"
}

# Place or remove a part through the harness command, and wait for its reply
# (bracketed by a fresh marker, as `read_info` is).
set_part() { # set_part X Y Z FACE PART
    local marker="it-part-$$-${RANDOM}-$1-$2-$3-$4-$5"
    say "integratedtunnels part $1 $2 $3 $4 $5"
    ctl chat "$marker" >/dev/null
    for _ in $(seq 1 40); do
        ctl wait 2 >/dev/null
        if ctl messages 100 | jq -e --arg m "$marker"             '(.chat | to_entries | map(select(.value | contains($m))) | .[0].key) as $i
             | $i != null and ([.chat[($i+1):($i+15)][] | select(contains("on the "))] | length) > 0' >/dev/null; then
            return 0
        fi
    done
    fail "no /integratedtunnels part reply"
}

# The slice-1 scene: a source chest, two cables (the second one with an exporter
# on its east face) and a sink chest beyond it.
SOURCE_X=$((X0 - 1))
CABLE_X=$X0
EXPORT_CABLE_X=$((X0 + 1))
SINK_X=$((X0 + 2))

scene() {
    set_block "$SOURCE_X" $Y 0 "minecraft:chest"
    set_block "$CABLE_X" $Y 0 "integratedtunnels:tunnel_cable[part_west=importer_item]"
    set_block "$EXPORT_CABLE_X" $Y 0 "integratedtunnels:tunnel_cable"
    set_block "$SINK_X" $Y 0 "minecraft:chest"
    ctl wait 10 >/dev/null
}

run() {
    arena

    echo "== A: a source chest drains into the network store, at a bounded rate =="
    scene
    ctl container "$SOURCE_X" $Y 0 minecraft:cobblestone "$SOURCE_FILL" >/dev/null
    ctl wait 2 >/dev/null
    info "$CABLE_X" $Y 0
    [ "$STORE" -gt 0 ] || fail "A: nothing reached the network store"
    [ "$STORE" -lt "$SOURCE_FILL" ] \
        || fail "A: the store holds all $SOURCE_FILL at once, so the rate is not bounded"
    [ "$STORE" -le "$RATE_BOUND" ] \
        || fail "A: the store holds $STORE items a few ticks in, above the 64-per-tick bound ($RATE_BOUND)"
    echo "  rate: $STORE items imported within ~15 ticks (64 per tick in the unit tests)" >&2

    echo "== A2: the source empties into the store =="
    ctl wait 120 >/dev/null
    info "$CABLE_X" $Y 0
    expect_store "$SOURCE_FILL" "A2"
    [ "$(total "$SOURCE_X" $Y 0)" = 0 ] || fail "A2: the source chest still holds $(total "$SOURCE_X" $Y 0)"
    case "$STATUS" in
        active) ;;
        *) fail "A2: the cable's status is '${STATUS:-}', expected active" ;;
    esac

    echo "== B: an exporter hands the store to a sink chest, and nothing is lost =="
    set_part "$EXPORT_CABLE_X" $Y 0 east exporter_item
    ctl wait 120 >/dev/null
    info "$EXPORT_CABLE_X" $Y 0
    local sink store_conserved
    sink=$(total "$SINK_X" $Y 0)
    store_conserved=$((STORE + sink))
    [ "$sink" -eq "$SINK_CAPACITY" ] \
        || fail "B: the sink holds $sink items, expected a full chest ($SINK_CAPACITY)"
    [ "$store_conserved" -eq "$EXPECT_CONSERVED" ] \
        || fail "B: store + sink = $store_conserved, expected $EXPECT_CONSERVED (nothing may be lost)"
    has "$SINK_X" $Y 0 minecraft:cobblestone "$SINK_CAPACITY"

    echo "== C: with the exporter removed the store grows and the sink stays put =="
    set_part "$EXPORT_CABLE_X" $Y 0 east none
    ctl container "$SOURCE_X" $Y 0 minecraft:cobblestone 64 >/dev/null
    ctl wait 40 >/dev/null
    info "$CABLE_X" $Y 0
    expect_store $((EXPECT_CONSERVED - SINK_CAPACITY + 64)) "C (the network store took the 64)"
    [ "$(total "$SOURCE_X" $Y 0)" = 0 ] || fail "C: the source chest still holds $(total "$SOURCE_X" $Y 0)"
    [ "$(total "$SINK_X" $Y 0)" = "$SINK_CAPACITY" ] \
        || fail "C: the sink changed without an exporter ($(total "$SINK_X" $Y 0))"

    echo "== D: with the importer removed a full source chest stays full =="
    set_part "$CABLE_X" $Y 0 west none
    ctl container "$SOURCE_X" $Y 0 minecraft:cobblestone 64 >/dev/null
    ctl wait 40 >/dev/null
    info "$CABLE_X" $Y 0
    expect_store $((EXPECT_CONSERVED - SINK_CAPACITY + 64)) "D (the stores did not change)"
    [ "$(total "$SOURCE_X" $Y 0)" = 64 ] \
        || fail "D: the source chest holds $(total "$SOURCE_X" $Y 0), expected the 64 an importer would have taken"

    echo "== E: both parts back, the source drains into the store again =="
    set_part "$CABLE_X" $Y 0 west importer_item
    set_part "$EXPORT_CABLE_X" $Y 0 east exporter_item
    ctl wait 60 >/dev/null
    info "$CABLE_X" $Y 0
    expect_store $((EXPECT_CONSERVED - SINK_CAPACITY + 128)) "E"
    [ "$(total "$SOURCE_X" $Y 0)" = 0 ] || fail "E: the source chest still holds $(total "$SOURCE_X" $Y 0)"

    echo "== F: a part on a face with no container behind it does nothing =="
    set_block "$((X0 + 6))" $((Y + 1)) 0 "integratedtunnels:tunnel_cable[part_west=importer_item]"
    ctl wait 20 >/dev/null
    info $((X0 + 6)) $((Y + 1)) 0
    expect_store 0 "F (a cable of its own is a network of its own)"

    echo "== H: a component-bearing stack is left in its chest (the store keeps only an id) =="
# The finding: the import saved only the item id and count, so a filled shulker or a
# damaged tool was rebuilt as a fresh stack. The importer now leaves it alone.
set_block "$((X0 + 10))" $((Y + 2)) 0 "minecraft:chest"
set_block "$((X0 + 11))" $((Y + 2)) 0 "integratedtunnels:tunnel_cable[part_west=importer_item]"
ctl wait 6
ctl command "execute in minecraft:the_end run item replace block $((X0 + 10)) $((Y + 2)) 0 container.0 with minecraft:diamond_pickaxe[minecraft:damage=10]" >/dev/null
ctl wait 4
ctl container "$((X0 + 10))" $((Y + 2)) 0 | jq -e '[.slots[]? | select(. != null and .item == "minecraft:diamond_pickaxe")] | length == 1' >/dev/null \
    || fail "H: the damaged pickaxe was not in the chest to begin with"
info $((X0 + 11)) $((Y + 2)) 0
expect_store 0 "H (a stack with components does not enter the store)"
[ "$(total "$((X0 + 10))" $((Y + 2)) 0)" = 1 ] \
    || fail "H: the chest lost its damaged pickaxe"
echo "  the damaged pickaxe stayed in the chest; the store holds 0" >&2

echo "== G: right-clicking a cable face with the Item Importer places the part =="
    # One block above the floor: the eye (feet + 1.62) then looks through the
    # 4 px core's west face, which a ray at floor level would pass under.
    set_block "$((X0 + 6))" $((Y + 1)) 4 "integratedtunnels:tunnel_cable"
    ctl command "item replace entity @s hotbar.0 with integratedtunnels:part_importer_item 2" >/dev/null
    ctl select 1 >/dev/null
    ctl wait 4 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((X0 + 5)).5 $Y 4.5" >/dev/null
    ctl wait 8 >/dev/null
    local reply aim
    aim=$(ctl look-at-block $((X0 + 6)) $((Y + 1)) 4 west 2>&1) \
        || fail "cannot aim at the cable's west face: $(tr -d '\n' <<<"$aim" | cut -c1-300)"
    reply=$(ctl interact $((X0 + 6)) $((Y + 1)) 4 west 2>&1) \
        || fail "ctl interact on the cable's west face failed: $(tr -d '\n' <<<"$reply" | cut -c1-300)"
    ctl wait 8 >/dev/null
    local parts
    info "$((X0 + 6))" $((Y + 1)) 4
    parts=$PARTS_LINE
    echo "  ${parts:-no reply}" >&2
    case "$parts" in
        *west=importer_item*) ;;
        *) fail "the right click did not place the importer on the west face: ${parts:-no reply}" ;;
    esac
    # The item ids are the jar's: the hotbar really holds one.
    ctl inventory | jq -e '[((.hotbar // [])[]), ((.main // [])[])]
        | map(select(. != null and .item == "integratedtunnels:part_importer_item")) | length > 0' >/dev/null \
        || fail "the Item Importer item id is not in the inventory"

    echo "== I: a full water cauldron drains into the fluid channel =="
    # The fluid scene stands at z=-4, clear of the item scene at z=0. The importer
    # is put on the cable only after the source is read back full, so the drain
    # itself (not the placement) is what empties the cauldron.
    local FZ=-4
    local FC1=$((X0 + 1)) FC2=$((X0 + 2)) FSOURCE=$X0 FSINK=$((X0 + 3))
    set_block "$FSOURCE" $Y $FZ "minecraft:water_cauldron[level=3]"
    set_block "$FC1" $Y $FZ "integratedtunnels:tunnel_cable"
    set_block "$FC2" $Y $FZ "integratedtunnels:tunnel_cable"
    set_block "$FSINK" $Y $FZ "minecraft:cauldron"
    cauldron_is "$FSOURCE" $Y $FZ "minecraft:water_cauldron[level=3]"
    cauldron_is "$FSINK" $Y $FZ "minecraft:cauldron"
    set_part "$FC1" $Y $FZ west importer_fluid
    ctl wait 40 >/dev/null
    info "$FC1" $Y $FZ
    expect_fluid 1000 "I (a full cauldron is 1000 mB)"
    [ "$STATUS" = active ] || fail "I: status is '${STATUS:-}', expected active on the fluid channel"
    cauldron_is "$FSOURCE" $Y $FZ "minecraft:cauldron"
    echo "  the source cauldron emptied into the channel" >&2

    echo "== J: a Fluid Exporter fills an empty cauldron from the channel =="
    set_part "$FC2" $Y $FZ east exporter_fluid
    ctl wait 60 >/dev/null
    info "$FC1" $Y $FZ
    expect_fluid 0 "J (all 1000 mB left the channel)"
    cauldron_is "$FSINK" $Y $FZ "minecraft:water_cauldron[level=3]"
    echo "  the sink cauldron holds the 1000 mB; the channel is empty" >&2

    echo "== K1: with the Fluid Exporter removed the channel keeps the fluid =="
    set_part "$FC2" $Y $FZ east none
    set_block "$FSINK" $Y $FZ "minecraft:cauldron"
    set_block "$FSOURCE" $Y $FZ "minecraft:water_cauldron[level=3]"
    ctl wait 40 >/dev/null
    info "$FC1" $Y $FZ
    expect_fluid 1000 "K1 (the importer is the only source, the channel keeps it)"
    cauldron_is "$FSINK" $Y $FZ "minecraft:cauldron"
    echo "  without the exporter nothing reached the sink" >&2

    echo "== K2: with the Fluid Importer removed a refilled source stays full =="
    set_part "$FC1" $Y $FZ west none
    set_block "$FSOURCE" $Y $FZ "minecraft:water_cauldron[level=3]"
    ctl wait 40 >/dev/null
    info "$FC1" $Y $FZ
    expect_fluid 1000 "K2 (the channel did not grow)"
    cauldron_is "$FSOURCE" $Y $FZ "minecraft:water_cauldron[level=3]"
    echo "  without the importer the source stayed full" >&2

    echo "== L: the two fluid part items exist and read back =="
    ctl command "item replace entity @s hotbar.7 with integratedtunnels:part_importer_fluid" >/dev/null
    ctl command "item replace entity @s hotbar.8 with integratedtunnels:part_exporter_fluid" >/dev/null
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[((.hotbar // [])[]), ((.main // [])[])]
        | map(select(. != null and .item == "integratedtunnels:part_importer_fluid")) | length > 0' >/dev/null \
        || fail "the Fluid Importer item id is not in the inventory"
    ctl inventory | jq -e '[((.hotbar // [])[]), ((.main // [])[])]
        | map(select(. != null and .item == "integratedtunnels:part_exporter_fluid")) | length > 0' >/dev/null \
        || fail "the Fluid Exporter item id is not in the inventory"
    echo "  both fluid part ids are real items" >&2

    echo "== M: the cable's loot table gives the cable =="
    # The block's self-drop is carried data; roll it through the game's loot path
    # so a table 26.3 could not read would show as an empty result, not as green JSON.
    ctl command "loot give @s loot integratedtunnels:blocks/tunnel_cable" >/dev/null
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[((.hotbar // [])[]), ((.main // [])[])]
        | map(select(. != null and .item == "integratedtunnels:tunnel_cable")) | length > 0' >/dev/null \
        || fail "the tunnel cable's loot table gave no tunnel cable"
    echo "  the loot table rolled a tunnel cable" >&2
}

launch
run
echo "PASS: an Item Importer pulled a chest into the tunnel network's store at a bounded rate, an Item Exporter handed it to a second chest without losing any (${EXPECT_CONSERVED}), a Fluid Importer drained a full cauldron into the network's fluid channel and a Fluid Exporter filled a second cauldron from it (1000 mB), and removing either part stopped its half of the loop"
