#!/usr/bin/env python3
# Example invocation in fish shell
# cargo build && sudo ./target/debug/framework_tool \
#   --driver portio --has-mec false --pd-ports 1 1 --pd-addrs 64 64 \
#   (./rgbkbd.py | string split ' ')

BRIGHTNESS = 1
RED    = int(0xFF * BRIGHTNESS) << 16
GREEN  = int(0xFF * BRIGHTNESS) << 8
BLUE   = int(0xFF * BRIGHTNESS)
CYAN   = GREEN + BLUE
YELLOW = RED + GREEN
PURPLE = BLUE + RED
WHITE  = RED + GREEN + BLUE

grid_4x4 = [
  [ YELLOW,    RED,    RED,    RED, YELLOW ],
  [    RED,  WHITE,  GREEN,  WHITE,    RED ],
  [    RED,  GREEN,   BLUE,  GREEN,    RED ],
  [    RED,  WHITE,  GREEN,  WHITE,    RED ],
  [ YELLOW,    RED,    RED,    RED, YELLOW ],
]

fan_8leds = [[
    # WHITE, CYAN, BLUE, GREEN, PURPLE, RED, YELLOW, WHITE
    RED, RED, RED, RED,
    GREEN, GREEN, GREEN, GREEN
]]

# colors = grid_4x4
colors = fan_8leds

print('--rgbkbd 0', end='')
for row in colors:
    for col in row:
      print(' ', end='')
      print(col, end='')
