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
# `GAME` is required (there is no port-built game any more); `--dev-mod` names
# the package folder so the launcher-built game reads its content and code.
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
#     (upstream's `ItemPart.useOn`).
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
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    args+=(--dev-mod "$PORT")
    echo "  launching $world ($GAME)" >&2
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    echo "  launched; waiting for the world" >&2
    ctl wait-ready 120
    ctl wait 40
    [ "$(ctl state | jq -r '.screen')" = playing ] || fail "the game is not playing"
    echo "  playing" >&2
}

block_state() { ctl block "$1" "$2" "$3" | jq -r '.state // empty'; }
block_name() { ctl block "$1" "$2" "$3" | jq -r '.block // empty'; }
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

# Chat lines in this fresh scratch world are newest first (`messages` says so);
# the buffer holds every line the run printed, so a *count* is how a new reply
# is told from an older one without guessing at tick timing on a loaded box.
count_lines() { ctl messages 60 | jq --arg p "$1" '[.chat[]? | select(contains($p))] | length'; }
newest_line() { ctl messages 60 | jq -r --arg p "$1" '.chat[]? | select(contains($p))' | head -1; }
# Wait until more than BEFORE lines contain PATTERN, then print the newest.
wait_reply() { # wait_reply PATTERN BEFORE
    local tries
    for tries in $(seq 1 40); do
        ctl wait 4 >/dev/null
        if [ "$(count_lines "$1")" -gt "$2" ]; then
            newest_line "$1"
            return 0
        fi
    done
    return 1
}

# `/integratedtunnels info X Y Z`: the parts line and the store line.
info() { # info X Y Z
    local before
    before=$(count_lines 'store ')
    say "integratedtunnels info $1 $2 $3"
    STORE_LINE=$(wait_reply 'store ' "$before") || fail "no /integratedtunnels info reply"
    PARTS_LINE=$(newest_line 'parts ')
    [ -n "$PARTS_LINE" ] || fail "no /integratedtunnels parts reply"
    [ -n "$STORE_LINE" ] || fail "no /integratedtunnels store reply"
    STORE=$(grep -o '[0-9]* items' <<<"$STORE_LINE" | head -1 | awk '{print $1}')
    STATUS=$(sed -n 's/.*status \([a-z]*\).*/\1/p' <<<"$STORE_LINE")
    [ -n "$STORE" ] || fail "the store line did not parse: $STORE_LINE"
    echo "  $PARTS_LINE" >&2
    echo "  $STORE_LINE" >&2
}

expect_store() { # expect_store VALUE LABEL
    [ "$STORE" = "$1" ] || fail "$2: the network store holds $STORE items, expected $1"
}

# Place or remove a part through the harness command, and wait for its reply.
set_part() { # set_part X Y Z FACE PART
    local before
    before=$(count_lines 'on the ')
    say "integratedtunnels part $1 $2 $3 $4 $5"
    wait_reply 'on the ' "$before" >/dev/null || fail "no /integratedtunnels part reply"
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
    local before parts
    before=$(count_lines 'store ')
    say "integratedtunnels info $((X0 + 6)) $((Y + 1)) 4"
    wait_reply 'store ' "$before" >/dev/null || fail "no /integratedtunnels info reply after the right click"
    parts=$(newest_line 'parts ')
    echo "  ${parts:-no reply}" >&2
    case "$parts" in
        *west=importer_item*) ;;
        *) fail "the right click did not place the importer on the west face: ${parts:-no reply}" ;;
    esac
    # The item ids are the jar's: the hotbar really holds one.
    ctl inventory | jq -e '[((.hotbar // [])[]), ((.main // [])[])]
        | map(select(. != null and .item == "integratedtunnels:part_importer_item")) | length > 0' >/dev/null \
        || fail "the Item Importer item id is not in the inventory"
}

launch
run
echo "PASS: an Item Importer pulled a chest into the tunnel network's store at a bounded rate, an Item Exporter handed it to a second chest without losing any (${EXPECT_CONSERVED}), and removing either part stopped its half of the loop"
