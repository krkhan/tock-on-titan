#!/bin/env python3

import argparse
import re

from collections import namedtuple

Section = namedtuple('Section', 'name, perms, begin, length')

SECTION_LAYOUT_REGEX: str = r'\s+(?P<section>\w+)\s+(?P<perms>\([rwx]+\)).*ORIGIN.*?(?P<begin>[0-9a-fx]+).*LENGTH.*?(?P<length>[0-9a-fx]+)'

def read_layout_file(path: str) -> dict[str, Section]:
  sections: dict[str, Section] = {}
  pattern = re.compile(SECTION_LAYOUT_REGEX, re.IGNORECASE)
  lines: list[str] = []
  with open(path, 'r') as f:
    lines = f.readlines()
  for line in lines:
    results = pattern.search(line)
    if results:
      sections[results.group('section')] = Section(name=results.group('section'), perms=results.group('perms'), begin=results.group('begin'), length=results.group('length'))
  return sections

def write_layout_file(path: str, updated_sections: dict[str, Section]):
  with open(path, 'r') as f:
    lines = f.readlines()
  updated_lines = []
  pattern = re.compile(SECTION_LAYOUT_REGEX, re.IGNORECASE)
  for line in lines:
    results = pattern.search(line)
    if not results:
      updated_lines.append(line)
      continue
    if results.group('section') in updated_sections:
      new_section = updated_sections[results.group('section')]
      replace_pattern = r'ORIGIN\s+?=\s+?[0-9a-fx]+'
      updated_line = re.sub(replace_pattern, 'ORIGIN = {}'.format(new_section.begin), line, flags=re.IGNORECASE)
      replace_pattern = r'LENGTH\s+?=\s+?[0-9a-fx]+'
      updated_line = re.sub(replace_pattern, 'LENGTH = {}'.format(new_section.length), updated_line, flags=re.IGNORECASE)
      updated_lines.append(updated_line)
  with open(path, 'w') as f:
    f.writelines(updated_lines)

def modify_chip_layout(path: str, delta: int):
  print('Updating {}'.format(path))
  sections = read_layout_file(path)

  rom_section = sections['rom']
  new_length = int(rom_section.length, 16) - delta
  rom_section_modified = Section(name=rom_section.name, perms=rom_section.perms, begin=rom_section.begin, length='{:#010x}'.format(new_length))
  sections['rom'] = rom_section_modified

  prog_section = sections['prog']
  new_begin = int(prog_section.begin, 16) - delta
  new_length = int(prog_section.length, 16) + delta
  prog_section_modified = Section(name=prog_section.name, perms=prog_section.perms, begin='{:#010x}'.format(new_begin), length='{:#010x}'.format(new_length))
  sections['prog'] = prog_section_modified

  write_layout_file(path, sections)

def modify_userspace_layout(path: str, delta: int):
  print('Updating {}'.format(path))
  sections = read_layout_file(path)

  flash_section = sections['FLASH']
  new_begin = int(flash_section.begin, 16) - delta
  new_length = int(flash_section.length, 16) + delta
  flash_section_modified = Section(name=flash_section.name, perms=flash_section.perms, begin='{:#010x}'.format(new_begin), length='{:#010x}'.format(new_length))
  sections['FLASH'] = flash_section_modified

  write_layout_file(path, sections)

def main():
  parser = argparse.ArgumentParser()
  parser.add_argument('-d', '--delta', help='Delta for kernel/app boundary; +ve increases app size, -ve increases kernel size', type=str)
  args = parser.parse_args()
  delta = 0
  if args.delta.startswith('0x') or args.delta.startswith('+0x') or args.delta.startswith('-0x'):
    delta = int(args.delta, 16)
  else:
    delta = int(args.delta)
  print('Delta: {:#X} bytes'.format(delta))

  modify_chip_layout('kernel/chip_layout_a.ld', delta)
  modify_chip_layout('kernel/chip_layout_b.ld', delta)

  modify_userspace_layout('userspace/layout_a.ld', delta)
  modify_userspace_layout('userspace/layout_b.ld', delta)

if __name__ == "__main__":
  main()

