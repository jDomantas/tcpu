screen_width = 64
screen_height = 48

min_scale = 7
max_scale = 14

def disk_width(scale):
    width = screen_width * scale - 41 - 4 - 2 * 2 * 5
    width = width // 2
    width = (width - 10) // 10 * 10 + 1
    return width

for scale in reversed(range(min_scale, max_scale)):
    next_scale = scale + 1
    width = screen_width * next_scale + 2 # screen
    width += disk_width(next_scale) # library
    width += 30 # gaps
    height = screen_height * next_scale + 2 # screen
    height += 41 # disk slots
    height += 30 # gaps
    print('@media (max-width: {}px), (max-height: {}px) {{'.format(width - 1, height - 1))
    print('    .diskLike {')
    print('         width: {}px;'.format(disk_width(scale)))
    print('    }')
    print('    #screen {')
    print('        width: {}px;'.format(scale * screen_width))
    print('        height: {}px;'.format(scale * screen_height))
    print('    }')
    print('}')
